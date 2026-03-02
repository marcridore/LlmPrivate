//! OpenClaw server: manages an OpenClaw gateway subprocess for AI agent integration.
//!
//! OpenClaw is a self-hosted Node.js gateway that bridges messaging platforms
//! (WhatsApp, Telegram, Discord) with AI agents. This module handles:
//! - Prerequisite detection (Node.js, OpenClaw CLI)
//! - Installation of Node.js and OpenClaw
//! - Gateway subprocess lifecycle (start/stop/restart/health)
//! - Configuration via OpenClaw CLI (models, providers, channels)
//!
//! Architecture mirrors VisionServer: RwLock<Option<Inner>> with async process mgmt.
//!
//! The OpenClaw gateway uses WebSocket RPC, not HTTP REST. All interactions
//! (health, provider config, WhatsApp) go through the `openclaw` CLI which
//! handles the WebSocket protocol internally.

use std::path::PathBuf;
use serde::Serialize;
use tokio::sync::RwLock;

use crate::error::AppError;

/// Windows: CREATE_NO_WINDOW flag prevents console windows from flashing
/// when spawning CLI subprocesses from a GUI application.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Default port for the OpenClaw gateway (loopback only).
const OPENCLAW_PORT: u16 = 18789;

/// Node.js version to install if not found.
const NODE_VERSION: &str = "22.14.0";

/// Manages the OpenClaw gateway child process.
pub struct OpenClawServer {
    inner: RwLock<Option<OpenClawInner>>,
}

struct OpenClawInner {
    child: tokio::process::Child,
    port: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupStatus {
    pub node_version: Option<String>,
    pub openclaw_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenClawStatus {
    pub running: bool,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QrResponse {
    pub qr_data_url: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaitResponse {
    pub connected: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatus {
    pub whatsapp_connected: bool,
    pub whatsapp_account_id: Option<String>,
}

impl OpenClawServer {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }

    // ── CLI helper ────────────────────────────────────────────────────

    /// Run an `openclaw` CLI command and return stdout on success.
    async fn run_cmd(args: &[&str]) -> Result<String, AppError> {
        let openclaw_bin = Self::find_openclaw_binary().ok_or_else(|| {
            AppError::OpenClaw("OpenClaw binary not found".to_string())
        })?;

        Self::run_cmd_with_bin(&openclaw_bin, args).await
    }

    /// Run an openclaw command using a specific binary path.
    ///
    /// On Windows, `.cmd` files must be run through `cmd.exe /c` since
    /// `CreateProcess` in GUI processes can't execute them directly.
    async fn run_cmd_with_bin(bin: &PathBuf, args: &[&str]) -> Result<String, AppError> {
        tracing::debug!("Running: {} {}", bin.display(), args.join(" "));

        let output = Self::spawn_openclaw_cmd(bin, args)
            .output()
            .await
            .map_err(|e| AppError::OpenClaw(format!("Failed to run openclaw: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first_arg = args.first().unwrap_or(&"");
            let detail = if !stderr.is_empty() {
                stderr.chars().take(500).collect::<String>()
            } else {
                stdout.chars().take(500).collect::<String>()
            };
            return Err(AppError::OpenClaw(format!(
                "openclaw {first_arg} failed: {detail}"
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run an `openclaw` CLI command with a timeout (seconds).
    async fn run_cmd_timeout(args: &[&str], timeout_secs: u64) -> Result<String, AppError> {
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            Self::run_cmd(args),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(AppError::OpenClaw(format!(
                "openclaw {} timed out after {}s",
                args.first().unwrap_or(&""),
                timeout_secs
            ))),
        }
    }

    /// Create a `tokio::process::Command` for an openclaw binary, handling
    /// Windows `.cmd` files (which need `cmd.exe /c`) and `.mjs` files
    /// (which need `node`).
    ///
    /// On Windows, adds `CREATE_NO_WINDOW` to prevent console flash from
    /// GUI processes, and handles extensionless files (npm POSIX shim) by
    /// looking up the `.mjs` entry point and running it via `node` directly.
    fn spawn_openclaw_cmd(bin: &PathBuf, args: &[&str]) -> tokio::process::Command {
        let ext = bin.extension().and_then(|e| e.to_str()).unwrap_or("");

        #[cfg(windows)]
        {
            if ext.eq_ignore_ascii_case("cmd") {
                // .cmd files must be run through cmd.exe on Windows GUI processes
                let mut cmd = tokio::process::Command::new("cmd.exe");
                cmd.arg("/c").arg(bin);
                for arg in args {
                    cmd.arg(arg);
                }
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());
                cmd.creation_flags(CREATE_NO_WINDOW);
                return cmd;
            }
            if ext.eq_ignore_ascii_case("mjs") || ext.eq_ignore_ascii_case("js") {
                // .mjs/.js entry points must be run through node
                let mut cmd = tokio::process::Command::new("node");
                cmd.arg(bin);
                for arg in args {
                    cmd.arg(arg);
                }
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());
                cmd.creation_flags(CREATE_NO_WINDOW);
                return cmd;
            }

            // Extensionless file (npm creates a POSIX shell shim called `openclaw`
            // without extension — it's NOT a valid Win32 executable and causes
            // error 193). Find the .mjs entry point and run it via node instead.
            if ext.is_empty() {
                tracing::warn!(
                    "spawn_openclaw_cmd: extensionless binary '{}', looking for .mjs fallback",
                    bin.display()
                );
                if let Some(mjs) = Self::find_mjs_entry_point() {
                    let mut cmd = tokio::process::Command::new("node");
                    cmd.arg(&mjs);
                    for arg in args {
                        cmd.arg(arg);
                    }
                    cmd.stdout(std::process::Stdio::piped());
                    cmd.stderr(std::process::Stdio::piped());
                    cmd.creation_flags(CREATE_NO_WINDOW);
                    return cmd;
                }
            }

            // Last resort: try direct execution with CREATE_NO_WINDOW
            let mut cmd = tokio::process::Command::new(bin);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            cmd.creation_flags(CREATE_NO_WINDOW);
            return cmd;
        }

        #[cfg(not(windows))]
        {
            if ext == "mjs" || ext == "js" {
                let mut cmd = tokio::process::Command::new("node");
                cmd.arg(bin);
                for arg in args {
                    cmd.arg(arg);
                }
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());
                return cmd;
            }

            let mut cmd = tokio::process::Command::new(bin);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            cmd
        }
    }

    /// Find the `openclaw.mjs` entry point directly in the npm global install dir.
    /// This is the most reliable way to run openclaw on Windows GUI processes,
    /// bypassing all `.cmd` shim issues.
    #[cfg(windows)]
    fn find_mjs_entry_point() -> Option<PathBuf> {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let mjs_path = PathBuf::from(&appdata)
                .join("npm")
                .join("node_modules")
                .join("openclaw")
                .join("openclaw.mjs");
            if mjs_path.exists() {
                tracing::debug!("Found openclaw.mjs at: {}", mjs_path.display());
                return Some(mjs_path);
            }
        }
        None
    }

    // ── Prerequisite detection ────────────────────────────────────────

    /// Check if Node.js and OpenClaw are installed on the system.
    pub async fn check_prerequisites(&self) -> SetupStatus {
        let node_version = Self::detect_node().await;
        let openclaw_version = Self::detect_openclaw().await;

        tracing::info!(
            "Prerequisites check: node={:?}, openclaw={:?}",
            node_version,
            openclaw_version
        );

        SetupStatus {
            node_version,
            openclaw_version,
        }
    }

    /// Detect Node.js version by running `node --version`.
    async fn detect_node() -> Option<String> {
        let mut cmd = tokio::process::Command::new("node");
        cmd.arg("--version")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output().await.ok()?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if version.starts_with('v') {
                Some(version)
            } else {
                Some(format!("v{}", version))
            }
        } else {
            None
        }
    }

    /// Detect OpenClaw version.
    ///
    /// Tries multiple strategies since the Tauri GUI process may not have
    /// `%APPDATA%\npm` in its PATH (unlike a terminal shell), and `.cmd`
    /// files need `cmd.exe /c` to execute from GUI processes:
    /// 1. `openclaw --version` from PATH
    /// 2. Known binary via `find_openclaw_binary()` (uses `spawn_openclaw_cmd` for .cmd/.mjs)
    /// 3. Direct `node <mjs_path> --version` (most reliable on Windows)
    /// 4. `npx openclaw --version` as last resort
    async fn detect_openclaw() -> Option<String> {
        // Strategy 1: try `openclaw` from PATH
        if let Some(version) = Self::try_version_with_path("openclaw").await {
            tracing::debug!("detect_openclaw: found via PATH");
            return Some(version);
        }

        // Strategy 2: try the known binary location (uses cmd.exe /c for .cmd files)
        if let Some(bin) = Self::find_openclaw_binary() {
            tracing::debug!("Trying openclaw at known path: {}", bin.display());
            let output = Self::spawn_openclaw_cmd(&bin, &["--version"])
                .output()
                .await;
            if let Ok(output) = output {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !version.is_empty() {
                        tracing::debug!("detect_openclaw: found via known binary");
                        return Some(version);
                    }
                }
            }
        }

        // Strategy 3: try node <mjs_path> directly (most reliable on Windows GUI)
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let mjs_path = PathBuf::from(&appdata)
                    .join("npm")
                    .join("node_modules")
                    .join("openclaw")
                    .join("openclaw.mjs");
                if mjs_path.exists() {
                    tracing::debug!("Trying node {} --version", mjs_path.display());
                    let mut cmd = tokio::process::Command::new("node");
                    cmd.arg(&mjs_path)
                        .arg("--version")
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::null());
                    cmd.creation_flags(CREATE_NO_WINDOW);
                    let output = cmd.output().await;
                    if let Ok(output) = output {
                        if output.status.success() {
                            let version =
                                String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if !version.is_empty() {
                                tracing::debug!("detect_openclaw: found via node + mjs");
                                return Some(version);
                            }
                        }
                    }
                }
            }
        }

        // Strategy 4: try npx
        if let Some(version) = Self::try_version_with_path("npx").await {
            tracing::debug!("detect_openclaw: found via npx");
            return Some(version);
        }

        None
    }

    /// Helper: run `<cmd> --version` (or `npx openclaw --version`) and return trimmed stdout.
    async fn try_version_with_path(cmd: &str) -> Option<String> {
        let args: Vec<&str> = if cmd == "npx" {
            vec!["openclaw", "--version"]
        } else {
            vec!["--version"]
        };

        let mut command = tokio::process::Command::new(cmd);
        command.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let output = command.output().await.ok()?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }

        None
    }

    // ── Installation ──────────────────────────────────────────────────

    /// Install Node.js on Windows (silent MSI install).
    pub async fn install_node(&self) -> Result<(), AppError> {
        // Check if already installed
        if Self::detect_node().await.is_some() {
            tracing::info!("Node.js already installed, skipping");
            return Ok(());
        }

        tracing::info!("Installing Node.js v{}", NODE_VERSION);

        #[cfg(windows)]
        {
            // Download Node.js MSI installer
            let msi_url = format!(
                "https://nodejs.org/dist/v{}/node-v{}-x64.msi",
                NODE_VERSION, NODE_VERSION
            );
            let temp_dir = std::env::temp_dir();
            let msi_path = temp_dir.join(format!("node-v{}-x64.msi", NODE_VERSION));

            tracing::info!("Downloading Node.js from: {}", msi_url);

            let client = reqwest::Client::new();
            let resp = client
                .get(&msi_url)
                .send()
                .await
                .map_err(|e| AppError::OpenClaw(format!("Failed to download Node.js: {e}")))?;

            if !resp.status().is_success() {
                return Err(AppError::OpenClaw(format!(
                    "Node.js download failed with status: {}",
                    resp.status()
                )));
            }

            let bytes = resp
                .bytes()
                .await
                .map_err(|e| AppError::OpenClaw(format!("Failed to read Node.js installer: {e}")))?;

            tokio::fs::write(&msi_path, &bytes)
                .await
                .map_err(|e| AppError::OpenClaw(format!("Failed to save installer: {e}")))?;

            tracing::info!(
                "Downloaded Node.js installer ({:.1} MB), running silent install...",
                bytes.len() as f64 / 1024.0 / 1024.0
            );

            // Run msiexec silently
            let mut msi_cmd = tokio::process::Command::new("msiexec");
            msi_cmd.args(["/i", &msi_path.to_string_lossy(), "/qn", "/norestart"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            msi_cmd.creation_flags(CREATE_NO_WINDOW);
            let output = msi_cmd.output().await
                .map_err(|e| AppError::OpenClaw(format!("Failed to run msiexec: {e}")))?;

            // Clean up installer
            let _ = tokio::fs::remove_file(&msi_path).await;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AppError::OpenClaw(format!(
                    "Node.js installation failed (exit {}): {}",
                    output.status,
                    stderr.chars().take(500).collect::<String>()
                )));
            }

            tracing::info!("Node.js installed successfully");

            // Verify installation (PATH may need refresh)
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if Self::detect_node().await.is_none() {
                tracing::warn!("Node.js installed but not in PATH yet — may need shell restart");
            }
        }

        #[cfg(not(windows))]
        {
            return Err(AppError::OpenClaw(
                "Automatic Node.js installation is only supported on Windows. Please install Node.js 22+ manually.".to_string(),
            ));
        }

        Ok(())
    }

    /// Install OpenClaw globally via npm.
    ///
    /// On Windows, GUI processes (like Tauri) can produce broken npm global installs
    /// where the entry point `openclaw.mjs` is missing despite exit code 0.
    /// We verify the critical file exists after install and retry once if needed.
    pub async fn install_openclaw(&self) -> Result<(), AppError> {
        // Check if already installed and working
        if Self::detect_openclaw().await.is_some() {
            tracing::info!("OpenClaw already installed, skipping");
            return Ok(());
        }

        tracing::info!("Installing OpenClaw via npm...");

        // Try install up to 2 times (initial + 1 retry with cache clean)
        for attempt in 1..=2 {
            if attempt == 2 {
                tracing::info!("Retrying install with npm cache clean...");
                let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };
                let mut clean_cmd = tokio::process::Command::new(npm_cmd);
                clean_cmd.args(["cache", "clean", "--force"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(windows)]
                clean_cmd.creation_flags(CREATE_NO_WINDOW);
                let _ = clean_cmd.output().await;
            }

            let result = Self::run_npm_install().await;
            if let Err(e) = result {
                if attempt == 2 {
                    return Err(e);
                }
                tracing::warn!("Install attempt {} failed: {}", attempt, e);
                continue;
            }

            // Verify the critical entry point file exists (catches broken installs)
            if Self::verify_openclaw_install() {
                // Now verify it actually runs
                if let Some(version) = Self::detect_openclaw().await {
                    tracing::info!("Verified OpenClaw version: {}", version);
                    return Ok(());
                }
            }

            tracing::warn!(
                "Install attempt {}: npm succeeded but openclaw is not working",
                attempt
            );

            if attempt == 1 {
                // Clean up broken install before retry
                let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };
                let mut uninstall_cmd = tokio::process::Command::new(npm_cmd);
                uninstall_cmd.args(["uninstall", "-g", "openclaw"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(windows)]
                uninstall_cmd.creation_flags(CREATE_NO_WINDOW);
                let _ = uninstall_cmd.output().await;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }

        Err(AppError::OpenClaw(
            "OpenClaw was installed but the binary is not working after 2 attempts. \
             Try running 'npm uninstall -g openclaw && npm install -g openclaw' \
             in a terminal, then click Re-check."
                .to_string(),
        ))
    }

    /// Run `npm install -g openclaw` with proper environment.
    async fn run_npm_install() -> Result<(), AppError> {
        let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };

        let mut cmd = tokio::process::Command::new(npm_cmd);
        cmd.args(["install", "-g", "openclaw", "--prefer-online"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output().await
            .map_err(|e| AppError::OpenClaw(format!("Failed to run npm: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::info!("npm install stdout: {}", stdout.chars().take(500).collect::<String>());
        if !stderr.is_empty() {
            tracing::info!("npm install stderr: {}", stderr.chars().take(500).collect::<String>());
        }

        if !output.status.success() {
            return Err(AppError::OpenClaw(format!(
                "npm install failed (exit {}): {}",
                output.status,
                stderr.chars().take(500).collect::<String>()
            )));
        }

        tracing::info!("npm install completed with exit code 0");
        Ok(())
    }

    /// Check that the critical `openclaw.mjs` file exists in the npm global install dir.
    fn verify_openclaw_install() -> bool {
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let mjs_path = PathBuf::from(&appdata)
                    .join("npm")
                    .join("node_modules")
                    .join("openclaw")
                    .join("openclaw.mjs");
                let exists = mjs_path.exists();
                tracing::info!(
                    "Verify openclaw.mjs at {}: {}",
                    mjs_path.display(),
                    if exists { "EXISTS" } else { "MISSING" }
                );
                return exists;
            }
            false
        }
        #[cfg(not(windows))]
        {
            // On non-Windows, just try to detect via version check
            true
        }
    }

    // ── Gateway lifecycle ─────────────────────────────────────────────

    /// Find the openclaw binary. Checks PATH first, then common install locations.
    /// Returns a `.cmd`, `.mjs`, or plain binary path. Callers should use
    /// `spawn_openclaw_cmd()` to execute the returned path correctly.
    fn find_openclaw_binary() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            // Check known paths FIRST (before `where`) to avoid getting the
            // extensionless POSIX shim that `where openclaw` returns on Windows.
            // The .mjs entry point is the most reliable for GUI processes.
            if let Ok(appdata) = std::env::var("APPDATA") {
                // Prefer .cmd shim (handled via cmd.exe /c in spawn_openclaw_cmd)
                let cmd_path = PathBuf::from(&appdata).join("npm").join("openclaw.cmd");
                if cmd_path.exists() {
                    return Some(cmd_path);
                }

                // Fallback: .mjs entry point (handled via node in spawn_openclaw_cmd)
                let mjs_path = PathBuf::from(&appdata)
                    .join("npm")
                    .join("node_modules")
                    .join("openclaw")
                    .join("openclaw.mjs");
                if mjs_path.exists() {
                    tracing::debug!("Using openclaw.mjs directly: {}", mjs_path.display());
                    return Some(mjs_path);
                }
            }

            // Last resort: try `where openclaw` from PATH
            // (may return the extensionless POSIX shim, but spawn_openclaw_cmd handles it)
            if let Ok(output) = std::process::Command::new("where")
                .arg("openclaw")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
            {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout);
                    let first_line = path.lines().next().unwrap_or("").trim();
                    if !first_line.is_empty() {
                        return Some(PathBuf::from(first_line));
                    }
                }
            }
        }

        #[cfg(not(windows))]
        {
            if let Ok(output) = std::process::Command::new("which")
                .arg("openclaw")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
            {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
        }

        None
    }

    /// Start the OpenClaw gateway as a subprocess.
    ///
    /// On Windows, prefers running `node openclaw.mjs` directly to avoid
    /// all `.cmd` shim issues (error 193 from extensionless POSIX shims,
    /// PATH not including npm dir, etc.).
    pub async fn start(&self) -> Result<u16, AppError> {
        // Stop any existing server first
        self.stop().await;

        let port = OPENCLAW_PORT;
        let port_str = port.to_string();
        // `gateway run` starts the gateway in the foreground (as a subprocess).
        // `gateway` alone is the command group and doesn't start anything.
        // `--force` is omitted because it requires `fuser`/`lsof` which aren't on Windows.
        let gateway_args = ["gateway", "run", "--port", &port_str, "--allow-unconfigured"];

        // Build the spawn command.
        // On Windows, prefer node + mjs directly (most reliable from GUI processes).
        let child;

        #[cfg(windows)]
        {
            let mjs_path = Self::find_mjs_entry_point();
            if let Some(ref mjs) = mjs_path {
                tracing::info!(
                    "Starting OpenClaw gateway via node: {} (port {})",
                    mjs.display(),
                    port
                );
                let mut cmd = tokio::process::Command::new("node");
                cmd.arg(mjs);
                for arg in &gateway_args {
                    cmd.arg(arg);
                }
                cmd.stdout(std::process::Stdio::null());
                cmd.stderr(std::process::Stdio::piped());
                cmd.creation_flags(CREATE_NO_WINDOW);

                child = cmd.spawn().map_err(|e| {
                    AppError::OpenClaw(format!(
                        "Failed to spawn OpenClaw gateway (node {}): {e}",
                        mjs.display()
                    ))
                })?;
            } else {
                // Fallback: use find_openclaw_binary + spawn_openclaw_cmd
                let openclaw_bin = Self::find_openclaw_binary().ok_or_else(|| {
                    AppError::OpenClaw(
                        "OpenClaw binary not found. Please install it first via the setup wizard."
                            .to_string(),
                    )
                })?;

                tracing::info!(
                    "Starting OpenClaw gateway (fallback): {} (port {})",
                    openclaw_bin.display(),
                    port
                );

                let mut cmd = Self::spawn_openclaw_cmd(&openclaw_bin, &gateway_args);
                cmd.stdout(std::process::Stdio::null());
                cmd.stderr(std::process::Stdio::piped());

                child = cmd.spawn().map_err(|e| {
                    AppError::OpenClaw(format!(
                        "Failed to spawn OpenClaw gateway ({}): {e}",
                        openclaw_bin.display()
                    ))
                })?;
            }
        }

        #[cfg(not(windows))]
        {
            let openclaw_bin = Self::find_openclaw_binary().ok_or_else(|| {
                AppError::OpenClaw(
                    "OpenClaw binary not found. Please install it first via the setup wizard."
                        .to_string(),
                )
            })?;

            tracing::info!(
                "Starting OpenClaw gateway: {} (port {})",
                openclaw_bin.display(),
                port
            );

            let mut cmd = Self::spawn_openclaw_cmd(&openclaw_bin, &gateway_args);
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::piped());

            child = cmd.spawn().map_err(|e| {
                AppError::OpenClaw(format!("Failed to spawn OpenClaw gateway: {e}"))
            })?;
        }

        tracing::info!(
            "OpenClaw gateway spawned (PID: {:?}), waiting for health...",
            child.id()
        );

        // Health poll via CLI: `openclaw gateway health --json --timeout 3000`
        // The CLI reads the gateway URL and auth token from the config file.
        let mut healthy = false;

        for attempt in 1..=30 {
            // up to ~60 seconds (each attempt ≈ 2s with timeout)
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let result = Self::run_cmd(
                &["gateway", "health", "--json", "--timeout", "3000"],
            )
            .await;

            match result {
                Ok(output) => {
                    tracing::info!("OpenClaw gateway healthy after attempt {}: {}", attempt, output.trim());
                    healthy = true;
                    break;
                }
                Err(_) => {
                    if attempt % 5 == 0 {
                        tracing::debug!(
                            "OpenClaw gateway not ready (attempt {})",
                            attempt
                        );
                    }
                }
            }
        }

        if !healthy {
            tracing::error!("OpenClaw gateway failed to become healthy within 60s");
            return Err(AppError::OpenClaw(
                "OpenClaw gateway failed to start within 60 seconds".to_string(),
            ));
        }

        *self.inner.write().await = Some(OpenClawInner { child, port });

        Ok(port)
    }

    /// Stop the OpenClaw gateway if running.
    pub async fn stop(&self) {
        let mut lock = self.inner.write().await;
        if let Some(mut inner) = lock.take() {
            tracing::info!("Stopping OpenClaw gateway...");
            let _ = inner.child.kill().await;
            let _ = inner.child.wait().await;
            tracing::info!("OpenClaw gateway stopped");
        }
    }

    /// Restart the OpenClaw gateway.
    pub async fn restart(&self) -> Result<u16, AppError> {
        self.stop().await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        self.start().await
    }

    /// Check if the gateway is currently running.
    pub async fn is_running(&self) -> bool {
        self.inner.read().await.is_some()
    }

    /// Get the port the gateway is listening on.
    pub async fn port(&self) -> Option<u16> {
        self.inner.read().await.as_ref().map(|i| i.port)
    }

    /// Get the current status of the OpenClaw gateway.
    pub async fn status(&self) -> OpenClawStatus {
        let lock = self.inner.read().await;
        match lock.as_ref() {
            Some(inner) => OpenClawStatus {
                running: true,
                port: Some(inner.port),
            },
            None => OpenClawStatus {
                running: false,
                port: None,
            },
        }
    }

    /// Perform a health check on the running gateway via CLI.
    pub async fn health_check(&self) -> bool {
        if self.port().await.is_none() {
            return false;
        }

        Self::run_cmd_timeout(
            &["gateway", "health", "--json", "--timeout", "5000"],
            10,
        )
        .await
        .is_ok()
    }

    // ── Provider configuration ────────────────────────────────────────

    /// Configure the LLM provider for OpenClaw via CLI commands.
    ///
    /// Uses:
    /// - `openclaw models set <provider>/<model>` to set the default model
    /// - `openclaw config set models.providers.<provider> '{json}'` to configure API key/base URL
    pub async fn configure_provider(
        &self,
        provider: &str,
        model: &str,
        api_key: &str,
    ) -> Result<(), AppError> {
        // Set the default model: `openclaw models set <provider>/<model>`
        let model_id = format!("{}/{}", provider, model);
        Self::run_cmd_timeout(&["models", "set", &model_id], 15).await?;
        tracing::info!("OpenClaw default model set to: {}", model_id);

        // Configure the provider with API key if provided
        if !api_key.is_empty() {
            let base_url = match provider {
                "anthropic" => "https://api.anthropic.com/v1",
                "openai" => "https://api.openai.com/v1",
                "ollama" => "http://127.0.0.1:11434/v1",
                _ => {
                    return Err(AppError::OpenClaw(format!(
                        "Unknown provider: {provider}"
                    )));
                }
            };

            // Build provider config JSON
            let config_json = serde_json::json!({
                "baseUrl": base_url,
                "apiKey": api_key,
                "models": []
            });

            let config_path = format!("models.providers.{}", provider);
            let config_value = config_json.to_string();

            Self::run_cmd_timeout(
                &["config", "set", &config_path, &config_value, "--strict-json"],
                15,
            )
            .await?;

            tracing::info!("OpenClaw provider {} configured with API key", provider);
        }

        Ok(())
    }

    // ── WhatsApp integration ──────────────────────────────────────────

    /// Start WhatsApp login flow via gateway RPC.
    ///
    /// Uses `openclaw gateway call web.login.start --json` to request a QR code.
    /// Falls back to `channels.login.start` if the method isn't found.
    pub async fn whatsapp_login_start(&self, force: bool) -> Result<QrResponse, AppError> {
        if self.port().await.is_none() {
            return Err(AppError::OpenClaw(
                "OpenClaw gateway is not running".to_string(),
            ));
        }

        let params = serde_json::json!({
            "channel": "whatsapp",
            "force": force
        });
        let params_str = params.to_string();

        // Try web.login.start first (AgentsGalaxy method name)
        let result = Self::run_cmd_timeout(
            &[
                "gateway",
                "call",
                "web.login.start",
                "--json",
                "--params",
                &params_str,
                "--timeout",
                "30000",
            ],
            35,
        )
        .await;

        let output = match result {
            Ok(out) => out,
            Err(_) => {
                // Fallback: try channels.login.start
                Self::run_cmd_timeout(
                    &[
                        "gateway",
                        "call",
                        "channels.login.start",
                        "--json",
                        "--params",
                        &params_str,
                        "--timeout",
                        "30000",
                    ],
                    35,
                )
                .await?
            }
        };

        // Parse the JSON response
        let json: serde_json::Value = serde_json::from_str(output.trim()).unwrap_or_else(|_| {
            serde_json::json!({ "message": output.trim() })
        });

        Ok(QrResponse {
            qr_data_url: json["qrDataUrl"]
                .as_str()
                .or_else(|| json["qr_data_url"].as_str())
                .or_else(|| json["result"]["qrDataUrl"].as_str())
                .map(String::from),
            message: json["message"]
                .as_str()
                .or_else(|| json["result"]["message"].as_str())
                .unwrap_or("QR code generated")
                .to_string(),
        })
    }

    /// Wait for the user to scan the WhatsApp QR code via gateway RPC.
    pub async fn whatsapp_login_wait(&self) -> Result<WaitResponse, AppError> {
        if self.port().await.is_none() {
            return Err(AppError::OpenClaw(
                "OpenClaw gateway is not running".to_string(),
            ));
        }

        // Long timeout — user needs time to scan QR
        let result = Self::run_cmd_timeout(
            &[
                "gateway",
                "call",
                "web.login.wait",
                "--json",
                "--timeout",
                "180000",
            ],
            190,
        )
        .await;

        match result {
            Ok(output) => {
                let json: serde_json::Value =
                    serde_json::from_str(output.trim()).unwrap_or_else(|_| {
                        serde_json::json!({ "connected": false, "message": output.trim() })
                    });

                Ok(WaitResponse {
                    connected: json["connected"]
                        .as_bool()
                        .or_else(|| json["result"]["connected"].as_bool())
                        .unwrap_or(false),
                    message: json["message"]
                        .as_str()
                        .or_else(|| json["result"]["message"].as_str())
                        .unwrap_or("completed")
                        .to_string(),
                })
            }
            Err(e) => {
                let msg = e.to_string();
                // Check for known error patterns
                if msg.contains("stream") && msg.contains("error") {
                    // 515 "Stream Errored" — QR scanned but needs restart
                    Ok(WaitResponse {
                        connected: false,
                        message: "stream_errored".to_string(),
                    })
                } else if msg.contains("timeout") || msg.contains("expired") {
                    // QR expired
                    Ok(WaitResponse {
                        connected: false,
                        message: "qr_expired".to_string(),
                    })
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Log out of WhatsApp via CLI.
    pub async fn whatsapp_logout(&self) -> Result<(), AppError> {
        if self.port().await.is_none() {
            return Err(AppError::OpenClaw(
                "OpenClaw gateway is not running".to_string(),
            ));
        }

        Self::run_cmd_timeout(
            &["channels", "logout", "--channel", "whatsapp"],
            15,
        )
        .await?;

        tracing::info!("WhatsApp logged out");
        Ok(())
    }

    /// Get the WhatsApp channel connection status via CLI.
    ///
    /// Uses `openclaw channels status --json` to query the gateway.
    pub async fn get_channel_status(&self) -> Result<ChannelStatus, AppError> {
        if self.port().await.is_none() {
            return Ok(ChannelStatus {
                whatsapp_connected: false,
                whatsapp_account_id: None,
            });
        }

        let output = match Self::run_cmd_timeout(
            &["channels", "status", "--json", "--timeout", "10000"],
            15,
        )
        .await
        {
            Ok(out) => out,
            Err(e) => {
                tracing::debug!("Channel status check failed: {}", e);
                return Ok(ChannelStatus {
                    whatsapp_connected: false,
                    whatsapp_account_id: None,
                });
            }
        };

        let json: serde_json::Value =
            serde_json::from_str(output.trim()).unwrap_or(serde_json::Value::Null);

        // Parse the channel status — format varies by OpenClaw version
        // Try common structures:
        // 1. { channels: [{ channel: "whatsapp", connected: true, ... }] }
        // 2. { whatsapp: { connected: true, accountId: "..." } }
        // 3. { channelAccounts: { whatsapp: [{ connected, accountId }] } }

        let mut connected = false;
        let mut account_id: Option<String> = None;

        // Try format 1: array of channels
        if let Some(channels) = json["channels"].as_array() {
            for ch in channels {
                if ch["channel"].as_str() == Some("whatsapp")
                    || ch["name"].as_str() == Some("whatsapp")
                {
                    connected = ch["connected"].as_bool().unwrap_or(false)
                        || ch["status"].as_str() == Some("connected");
                    account_id = ch["accountId"]
                        .as_str()
                        .or_else(|| ch["account_id"].as_str())
                        .map(String::from);
                    break;
                }
            }
        }

        // Try format 2: direct whatsapp key
        if !connected {
            if let Some(wa) = json.get("whatsapp") {
                connected = wa["connected"].as_bool().unwrap_or(false)
                    || wa["status"].as_str() == Some("connected");
                account_id = wa["accountId"]
                    .as_str()
                    .or_else(|| wa["account_id"].as_str())
                    .map(String::from);
            }
        }

        // Try format 3: channelAccounts
        if !connected {
            if let Some(accounts) = json["channelAccounts"]["whatsapp"].as_array() {
                if let Some(first) = accounts.first() {
                    connected = first["connected"].as_bool().unwrap_or(false);
                    account_id = first["accountId"].as_str().map(String::from);
                }
            }
        }

        // Also check if WhatsApp credentials exist on disk as a hint
        if !connected && account_id.is_none() {
            if let Ok(home) = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
            {
                let creds_path = PathBuf::from(&home)
                    .join(".openclaw")
                    .join("credentials")
                    .join("whatsapp")
                    .join("default")
                    .join("creds.json");
                if creds_path.exists() {
                    // Credentials exist — WhatsApp was previously linked
                    // (may auto-connect when gateway starts)
                    account_id = Some("(saved session)".to_string());
                }
            }
        }

        Ok(ChannelStatus {
            whatsapp_connected: connected,
            whatsapp_account_id: account_id,
        })
    }
}

impl Drop for OpenClawServer {
    fn drop(&mut self) {
        // Best-effort synchronous cleanup — kill the child process
        let inner_opt = self.inner.get_mut();
        if let Some(inner) = inner_opt.take() {
            if let Some(id) = inner.child.id() {
                #[cfg(windows)]
                {
                    // Use /T to kill the entire process tree (Node.js spawns child processes)
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/T", "/PID", &id.to_string()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("kill")
                        .arg("-9")
                        .arg(id.to_string())
                        .status();
                }
            }
        }
    }
}

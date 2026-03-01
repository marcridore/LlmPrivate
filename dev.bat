@echo off
set LIBCLANG_PATH=C:\Program Files\LLVM\bin
set CMAKE=C:\Program Files\CMake\bin\cmake.exe
set CUDA_PATH=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1
set CUDACXX=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1\bin\nvcc.exe
set CUDAFLAGS=--allow-unsupported-compiler
set CMAKE_GENERATOR=Ninja
set PATH=C:\Program Files\CMake\bin;C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1\bin;C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1\bin\x64;C:\Program Files\Microsoft Visual Studio\18\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja;%PATH%

if not "%1"=="--gpu" goto cpu

echo [LlmPrivate] Building with CUDA GPU acceleration...
echo [LlmPrivate] CUDA: %CUDA_PATH%
npm run tauri dev -- --features cuda
goto end

:cpu
echo [LlmPrivate] Building CPU-only...
npm run tauri dev

:end

import { useEffect, useState } from "react";
import { codeToHtml } from "shiki";

interface CodeBlockProps {
  language: string;
  code: string;
}

export function CodeBlock({ language, code }: CodeBlockProps) {
  const [html, setHtml] = useState<string>("");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    codeToHtml(code, {
      lang: language,
      theme: "github-dark",
    })
      .then(setHtml)
      .catch(() => {
        // If the language isn't supported, just render as text
        setHtml(
          `<pre class="shiki"><code>${code
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")}</code></pre>`
        );
      });
  }, [code, language]);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative group my-4 rounded-lg overflow-hidden">
      <div className="flex items-center justify-between bg-zinc-800 px-4 py-2 text-xs text-zinc-400">
        <span>{language}</span>
        <button
          onClick={handleCopy}
          className="hover:text-white transition-colors"
        >
          {copied ? "Copied!" : "Copy"}
        </button>
      </div>
      <div
        className="overflow-x-auto text-sm [&_pre]:!m-0 [&_pre]:!rounded-none [&_pre]:!p-4"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}

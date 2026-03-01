import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { CodeBlock } from "./CodeBlock";

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      children={content}
      components={{
        code({ className, children, ...props }) {
          const match = /language-(\w+)/.exec(className || "");
          const isInline = !match;

          if (isInline) {
            return (
              <code
                className="bg-secondary px-1.5 py-0.5 rounded text-sm font-mono"
                {...props}
              >
                {children}
              </code>
            );
          }

          return (
            <CodeBlock
              language={match[1]}
              code={String(children).replace(/\n$/, "")}
            />
          );
        },
        table({ children }) {
          return (
            <div className="overflow-x-auto my-4">
              <table className="min-w-full border-collapse border border-border text-sm">
                {children}
              </table>
            </div>
          );
        },
        th({ children }) {
          return (
            <th className="border border-border px-3 py-1.5 bg-muted text-left font-medium">
              {children}
            </th>
          );
        },
        td({ children }) {
          return (
            <td className="border border-border px-3 py-1.5">{children}</td>
          );
        },
        a({ href, children }) {
          return (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              className="text-blue-400 underline hover:text-blue-300"
            >
              {children}
            </a>
          );
        },
      }}
    />
  );
}

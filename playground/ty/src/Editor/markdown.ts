function getLanguageTag(filename: string): string {
  const extension = filename.split(".").pop()?.toLowerCase();
  switch (extension) {
    case "py":
    case "pyi":
      return "py";
    case "json":
      return "json";
    case "toml":
      return "toml";
    default:
      return extension ?? "";
  }
}

export function generatePlaygroundMarkdownLink(shareUrl: string): string {
  return `[Playground](${shareUrl})`;
}

export function generatePlaygroundMarkdown(
  files: { [name: string]: string },
  shareUrl: string,
): string {
  let markdown = "";
  const showFilenames = Object.keys(files).length > 1;

  for (const [filename, content] of Object.entries(files)) {
    if (showFilenames) {
      markdown += `\`${filename}\`:

`;
    }

    markdown += `\`\`\`${getLanguageTag(filename)}
${content}
\`\`\`

`;
  }

  markdown += `[Playground](${shareUrl})
`;

  return markdown;
}

function cleanupClipboardText(targetSelector) {
  const targetElement = document.querySelector(targetSelector);

  const excludedClasses = ["gp", "go"];

  const clipboardText = Array.from(targetElement.childNodes)
    .filter(
      (node) =>
        !excludedClasses.some((className) =>
          node?.classList?.contains(className),
        ),
    )
    .map((node) => node.textContent)
    .filter((s) => s !== "");
  return clipboardText.join("").trim();
}

function setCopyText() {
  const attr = "clipboardText";
  const elements = document.querySelectorAll(
    'button[data-clipboard-target$="code"]',
  );

  if (elements.length === 0) {
    return;
  }

  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (
        entry.intersectionRatio > 0 &&
        entry.target.dataset[attr] === undefined
      ) {
        entry.target.dataset[attr] = cleanupClipboardText(
          entry.target.dataset.clipboardTarget,
        );
      }
    });
  });

  elements.forEach((elt) => {
    observer.observe(elt);
  });
}

document$.subscribe(function () {
  setCopyText();
});


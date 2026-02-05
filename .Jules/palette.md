## 2024-05-22 - Keyboard Navigation in Custom UIs
**Learning:** Custom interactive elements (like icon-only buttons and collapsible headers) often lose default browser focus rings when styled with Tailwind, or the default contrast is insufficient against dark backgrounds. Explicitly adding `focus-visible:ring` is crucial for keyboard users to navigate lists.
**Action:** Always add `focus-visible:ring-2` and `focus-visible:outline-none` to interactive elements in custom lists and headers.

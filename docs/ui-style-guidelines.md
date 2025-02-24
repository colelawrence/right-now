# UI Style Guidelines

## 1. Tailwind Setup & Organization

- Use consistent naming: Group utility classes logically. For example, use `bg-primary`, `text-primary`, or `border-primary` for your main brand color.
- Create design tokens: Define color, spacing, and typography tokens in your Tailwind config so they're easy to update globally.
- Layer utilities carefully: Start with base styles (e.g., `bg-white`, `text-base`) and apply modifiers (e.g., `hover:bg-blue-200`) for states.

## 2. Color Palette

- Bright, but harmonious: Limit your color palette to a few vibrant accents, ensuring overall consistency.
- Accent backgrounds: Use bright background utility classes (e.g., `bg-yellow-300`) for key elements or trackers, but keep surrounding elements neutral (e.g., `bg-gray-50`) to emphasize readability.
- High contrast: Ensure text has sufficient contrast against its background for accessibility and clarity.

## 3. Spacing & Layout

- Consistent padding & margins: Define standardized scales (e.g., `p-4`, `p-6`) to maintain uniform spacing.
- Compact state: Use tighter spacing (`p-2`, `py-1`) in smaller components so it remains visually appealing in a more constrained layout.
- Wide borders & outlines: Incorporate visible borders (`border-2`, `border-4`) for emphasis when the app is in a compact state.

## 4. Typography

- Establish hierarchy: Use Tailwind's text size utilities (`text-sm`, `text-base`, `text-lg`) consistently.
- Line height: Maintain adequate line spacing for readability (`leading-relaxed`, `leading-snug`) based on context.
- Limit fonts: Stick to 1–2 typefaces. Use weights (`font-semibold`, `font-bold`) sparingly to highlight only crucial text.

## 5. Interactive Elements

- Clear states: Use hover, focus, and active classes (`hover:bg-opacity-75`, `focus:outline-none`) to indicate interactivity.
- Touch-friendly targets: Provide enough padding (`py-2 px-4`) to ensure buttons and clickable elements are easy to tap.
- Smooth transitions: Use Tailwind's transition utilities (`transition-colors`, `transition-opacity`) to achieve a polished feel.

## 6. Visual Consistency

- Rounded corners: For a friendly, modern look, use `rounded-md` or `rounded-lg`. Stay consistent across components.
- Icon & image styling: Keep icons and imagery aligned (e.g., `flex items-center`) with consistent sizing (`w-5 h-5`, or `w-6 h-6`).
- Minimalistic approach: Balance bright highlights with whitespace and neutral sections. Don't overload the design with too many vibrant elements.

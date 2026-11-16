# TerraMars interface rules

These rules keep the desktop UI visually consistent. Values live in
`src/ui/style.rs`; this document explains when to use each one.

## Spacing scale

| Token | Value | Use |
| --- | --- | --- |
| `SPACE_TIGHT` | 8 px | Inside one control cluster: speed stepper, icon toolbar, segmented switch. |
| `SPACE` | 12 px | Between sibling controls, rows, and stacked sections in a panel. |
| `SPACE_LOOSE` | 20 px | Between top-level groups: home cards, log cards, forecast groups. |
| `INSET` | 16 px | Padding inside cards, popovers and the header. |
| `HEADER_INSET` | 12 px | Margin between the floating header and the window edges. |

The floating header uses `HEADER_INSET` on **every** side, so its top, left and
right gaps match. Content below it starts at `CONTENT_TOP`, which is the header
margin plus the header height plus the same margin again.

## Rules

1. Never use an arbitrary pixel gap. Pick the token for the relationship:
   sibling controls use `SPACE`, a tight control cluster uses `SPACE_TIGHT`,
   unrelated groups use `SPACE_LOOSE`.
2. Panels and cards pad with `INSET`; rows inside them are separated by `SPACE`.
3. Edge-anchored overlays (top-left card, Info, planner, map controls) all sit
   `HEADER_INSET` from the window edges or `CONTENT_TOP` below the header.
4. Every appearance and disappearance goes through `animation::Fade`, and a
   surface that is leaving becomes click-through.
5. Text uses Departure Mono for body copy and Orbitron for headings. The clock's
   primary line uses a multiple of the font's 11 px design grid.

## Motion

- One shared easing (`animation::ease_out`) and one set of durations.
- Size animations start from a measured height, never from a fixed maximum, so
  the card changes size on every frame instead of moving only at the end.

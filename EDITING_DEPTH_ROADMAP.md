# wcanvas Editing Depth Roadmap

## Context

wcanvas is a Rust/WebGPU whiteboard app (Excalidraw-inspired) that currently functions as a **draw-only tool**. Users can create 8 shape types, single-select and drag elements, erase, and undo the last addition — but cannot modify any property after creation, resize, multi-select, reorder, or duplicate. This roadmap addresses the editing depth gap to transform wcanvas from a sketch pad into a usable whiteboard editor.

---

## 1. Current Editing Model

### What users CAN do
- **Create**: 8 primitives (Stroke, Line, Rectangle, Circle, Diamond, Arrow, Text, TextBox) via click-drag/type
- **Select**: Single element via Select tool (click); drag to move
- **Erase**: Click element with Eraser tool to delete (no undo)
- **Undo/Redo**: Pop/push last created element only (not moves, deletions, or property changes)
- **Navigate**: Pan (shift+drag, middle-click), Zoom (wheel, pinch, Ctrl+/-), clamped 0.1x–10x
- **Style at creation**: Choose from 6 preset colors; rough/sketchy style auto-applied
- **Shortcuts**: Keys 1–9 for tools, Ctrl+Z/Y for undo/redo

### What users CANNOT do
- Change color, fill, stroke width, or size of existing elements
- Multi-select, group, duplicate, or copy/paste
- Resize or rotate elements
- Reorder layers (z-order)
- Use custom colors beyond 6 presets
- Edit text after committing (no cursor movement, no re-editing)
- Snap or align elements
- Delete selected elements via keyboard (only Eraser tool)
- Save/load drawings

---

## 2. Structural Problems Blocking Feature Development

These must be resolved before or alongside editing features — every subsequent feature depends on them.

### A. Vec-Index Identity (`state.rs:76`, `event_handler.rs:943`)
- `selected_element: Option<usize>` — element identity is a Vec position
- Any deletion, reorder, or z-change invalidates indices
- **Fix**: Wrapper struct `Element { id: ElementId, shape: DrawingElement }` with `AtomicU64` counter. Unifies with TextBox's existing `id: u64`.

### B. No Action/Command Undo (`event_handler.rs:408-427`)
- Undo = `elements.pop()` → `redo_stack.push()`. Only undoes "add element"
- Moves mutate in-place with no record. Eraser deletes with no undo path
- **Fix**: `Action` enum (`Add`, `Remove`, `Move`, `ModifyProperty`, `Batch`) with `undo_stack: Vec<Action>` + `redo_stack: Vec<Action>`

### C. Selection is Transient (`event_handler.rs:189`)
- Selection cleared on mouse-up after drag. No "selected but idle" state
- Highlight is visual-only overlay — no handles, no property panel
- **Fix**: Persistent selection cleared by click-on-empty or Escape. New `SelectionState` struct.

### D. Event Handler Monolith (`event_handler.rs` — 952 lines)
- All input logic in one file; will grow significantly with editing features
- Consider splitting: `select_handler.rs`, `draw_handler.rs`, `keyboard_handler.rs`

---

## 3. Ranked Feature Roadmap

### Phase 0: Foundational Refactors (prerequisites)

| ID | Feature | Files |
|----|---------|-------|
| 0A | **Stable Element IDs** — `Element` wrapper, ID-based selection | `drawing.rs`, `state.rs`, `app_state.rs`, `event_handler.rs`, `update_logic.rs` |
| 0B | **Action-based Undo/Redo** — command pattern replacing push/pop | New `history.rs`, `app_state.rs`, `event_handler.rs` |
| 0C | **Persistent Selection** — survives mouse-up, Escape to deselect | `state.rs`, `event_handler.rs` |
| 0D | **Bounding Box method** — `DrawingElement::bounding_box() -> ([f32;2], [f32;2])` | `drawing.rs` |

### Phase 1: Property Editing — *highest value/cost ratio*

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 1.1 | **Change color of selected element** | Very High | Low | `event_handler.rs` (color click handler, lines 92-97) |
| 1.2 | **Toggle fill on selected shape** | High | Medium | `event_handler.rs`, `update_logic.rs`, `rough.rs` (rough fill = hatch patterns) |
| 1.3 | **Change stroke width of selected** | High | Low | `event_handler.rs` |
| 1.4 | **Delete selected via keyboard** (Del/Backspace) | Very High | Low | `event_handler.rs` |

**UX**: When element is selected, clicking a color swatch applies to selection (not just `current_color`). `F` key toggles fill. Property changes record `ModifyProperty` actions for undo.

### Phase 2: Resize Handles — *transforms the select tool*

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 2.1 | **8-handle bounding box on selection** | Very High | High | `event_handler.rs`, `update_logic.rs`, `state.rs` |
| 2.2 | **Shift-drag for aspect-ratio lock** | Medium | Low (after 2.1) | `event_handler.rs` |

**UX**: 8 small squares at corners/midpoints of bounding box. Handle hit-test takes priority over element hit-test (handles are screen-space). Dragging handles scales the shape: Rectangle/Diamond change `size`, Circle changes `radius`, Stroke scales all points relative to center, Line/Arrow move endpoints.

**Rotation deferred** — requires adding `rotation: f32` to every element, OBB hit-testing, shader changes, and rotated rough seeds. High cost, moderate value at this stage.

### Phase 3: Multi-Select & Bulk Operations

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 3.1 | **Multi-select** (Shift+click, rubber-band drag) | High | Medium | `event_handler.rs`, `state.rs` |
| 3.2 | **Duplicate** (Ctrl+D) | High | Low | `event_handler.rs` |
| 3.3 | **Copy/Paste** (Ctrl+C/V, internal clipboard) | High | Medium | `event_handler.rs`, new clipboard module |

**UX**: `selected_elements: HashSet<ElementId>`. Shift+click toggles. Drag on empty starts rubber-band rectangle. All property changes and transforms apply to entire selection. Duplicate clones with offset + new IDs, recorded as `Batch` action.

### Phase 4: Layer Ordering

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 4.1 | **Bring to front / Send to back** | High | Low | `event_handler.rs` |
| 4.2 | **Bring forward / Send backward** | Medium | Low | `event_handler.rs` |

**UX**: `]`/`[` for forward/backward, `Ctrl+]`/`Ctrl+[` for front/back. Records `ReorderElement` action. Trivial with stable IDs — just rearrange the Vec.

### Phase 5: Custom Color Picker — *addresses todo.txt request*

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 5.1 | **HSV color picker widget** | High | Medium | `ui.rs` (self-contained new widget) |

**UX**: Click "+" swatch or long-press a swatch to open HSV rectangle + hue slider. Rendered via UI pipeline. Outputs `[f32; 4]` feeding into `current_color` or property-edit path. Entirely contained in `ui.rs`.

### Phase 6: Snapping & Alignment

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 6.1 | **Snap to grid** | Medium | Medium | `event_handler.rs`, `update_logic.rs` |
| 6.2 | **Smart guides** (snap to other element edges/centers) | High | High | New module |
| 6.3 | **Align selection** (left/center/right/top/middle/bottom) | Medium | Medium | `event_handler.rs` |

### Phase 7: Text Editing — *high value but very high cost*

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 7.1 | **Cursor position in text** (arrow keys) | High | High | `event_handler.rs`, `text_renderer.rs` |
| 7.2 | **Multi-line text** (Enter inserts newline) | High | High | `drawing.rs`, `text_renderer.rs` |
| 7.3 | **Wire TextBox type** (Text tool creates TextBox, double-click to re-edit) | Medium | Medium | `event_handler.rs`, `drawing.rs` |
| 7.4 | **Text selection** (Shift+arrows, click-drag) | Medium | Very High | `text_renderer.rs` |

**Why last**: The current MSDF text atlas doesn't track glyph positions for cursor placement. Every text editing feature requires mapping pixel positions ↔ character indices, which means glyph-position tracking in `TextRenderer`. The `TextInput` struct (state.rs) has no `cursor_pos` — only append and backspace. This is a significant subsystem rewrite.

### Phase 8: Grouping

| ID | Feature | User Value | Complexity | Files |
|----|---------|-----------|------------|-------|
| 8.1 | **Group/Ungroup** (Ctrl+G / Ctrl+Shift+G) | Medium | High | `drawing.rs`, `event_handler.rs`, `update_logic.rs` |

Requires `Group { id, children: Vec<ElementId> }` — groups act as single elements for selection/transform/z-order but expand for rendering. Benefits from all prior phases being stable.

---

## 4. Suggested UX Model for Editing Interactions

1. **Tool mode vs. Select mode**: Tool keys (2–9) enter creation mode. Key `1` or pointer icon enters Select mode. All editing happens in Select mode.

2. **Selection lifecycle**: Click selects → element shows bounding box with handles → click empty or Escape deselects. Shift+click for multi-select. Drag on empty for rubber-band.

3. **Context-sensitive toolbar actions**: When selection active, color swatches apply to selection. Property bar shows fill toggle, stroke slider, z-order buttons. When no selection, swatches set `current_color` for next creation.

4. **Double-click to deep-edit**: Double-click text enters editing mode (cursor appears). Double-click group enters the group (selects children).

5. **All mutations undoable**: Ctrl+Z reverses any action (create, delete, move, resize, property change, reorder). Ctrl+Y redoes.

---

## 5. Architectural Utilities to Add

| Utility | Purpose | Location |
|---------|---------|----------|
| `Element` wrapper struct | Stable ID + shape data | `drawing.rs` |
| `ElementId` newtype + counter | Unique identity | `drawing.rs` |
| `DrawingElement::bounding_box()` | Resize handles, rubber-band, snapping, alignment | `drawing.rs` |
| `DrawingElement::set_color()`, `set_fill()`, `set_stroke_width()` | Property mutation across variants | `drawing.rs` |
| `Action` enum + History | Undo/redo for all operations | New `history.rs` |
| `SelectionState` struct | IDs, handle positions, active handle | `state.rs` |
| `fn find_by_id()` helper | ID-based element lookup | `app_state.rs` or `drawing.rs` |

---

## 6. Verification Plan

After each phase:
- **Phase 0**: Create elements → delete one → verify indices are stable. Undo move → element returns to original position. Select element → release mouse → element stays selected.
- **Phase 1**: Select rectangle → click red swatch → rectangle turns red → Ctrl+Z → reverts to original color. Press F → fill toggles.
- **Phase 2**: Select element → 8 handles visible → drag corner handle → element resizes → Ctrl+Z → reverts size.
- **Phase 3**: Shift+click 3 elements → drag → all move together → Ctrl+D → duplicates all 3.
- **Phase 4**: Select element → press `]` → element moves forward in z-order visually.
- Build for both native (`cargo run`) and wasm (`build_run_web.sh`) to verify cross-platform.

---

## 7. Implementation Priority Summary

**Do first (Phases 0–1)**: Foundation + property editing. ~4 weeks of work. Transforms the app from draw-only to an actual editor. Highest ROI.

**Do next (Phases 2–4)**: Resize + multi-select + z-order. ~4 weeks. Makes the editor feel professional.

**Do later (Phases 5–8)**: Color picker, snapping, text editing, grouping. ~6 weeks. Polish features that add completeness but have diminishing returns or high complexity.

**Key tradeoff**: Rough fill rendering (Phase 1.2) is the most complex item in the early phases — it requires hatch-pattern generation in `rough.rs`. Consider shipping fill for clean/SDF shapes first (already supported in the SDF shader) and adding rough fill as a fast-follow.

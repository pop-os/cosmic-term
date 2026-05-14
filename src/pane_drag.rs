// SPDX-License-Identifier: GPL-3.0-only

// Pane-drag-from-anywhere support for the pane_grid. Triggered by a
// configurable modifier + left-click, since iced's built-in pane drag
// only fires from a TitleBar pick area (which we don't render).

use cosmic::Renderer;
use cosmic::iced::core::{
    Border, Color, Element, Length, Point, Rectangle, Size,
    layout::{self, Layout},
    mouse,
    renderer::{self, Quad, Renderer as _},
    widget::{Tree, Widget},
};
use cosmic::theme::Theme;
use cosmic::widget::pane_grid;

// Flattened mirror of pane_grid::Region; derives PartialEq for
// change-detection while tracking the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropRegion {
    Center,
    Top,
    Right,
    Bottom,
    Left,
}

impl DropRegion {
    // Mirrors iced's hovered_region heuristic: left/right thirds win over
    // top/bottom thirds; the center column has its own top/center/bottom.
    pub fn from_local_position(local: Point, bounds: Size) -> Self {
        if local.x < bounds.width / 3.0 {
            Self::Left
        } else if local.x > 2.0 * bounds.width / 3.0 {
            Self::Right
        } else if local.y < bounds.height / 3.0 {
            Self::Top
        } else if local.y > 2.0 * bounds.height / 3.0 {
            Self::Bottom
        } else {
            Self::Center
        }
    }

    pub fn preview_bounds(self, bounds: Rectangle) -> Rectangle {
        match self {
            Self::Center => bounds,
            Self::Top => Rectangle {
                height: bounds.height / 2.0,
                ..bounds
            },
            Self::Bottom => Rectangle {
                y: bounds.y + bounds.height / 2.0,
                height: bounds.height / 2.0,
                ..bounds
            },
            Self::Left => Rectangle {
                width: bounds.width / 2.0,
                ..bounds
            },
            Self::Right => Rectangle {
                x: bounds.x + bounds.width / 2.0,
                width: bounds.width / 2.0,
                ..bounds
            },
        }
    }
}

impl From<DropRegion> for pane_grid::Region {
    fn from(region: DropRegion) -> Self {
        match region {
            DropRegion::Center => pane_grid::Region::Center,
            DropRegion::Top => pane_grid::Region::Edge(pane_grid::Edge::Top),
            DropRegion::Right => pane_grid::Region::Edge(pane_grid::Edge::Right),
            DropRegion::Bottom => pane_grid::Region::Edge(pane_grid::Edge::Bottom),
            DropRegion::Left => pane_grid::Region::Edge(pane_grid::Edge::Left),
        }
    }
}

// Some((parent_axis, source_first)) when source and target are direct
// children of the same Split. source_first means source is the left/top
// child. None otherwise.
pub fn sibling_split_info(
    layout: &pane_grid::Node,
    source: pane_grid::Pane,
    target: pane_grid::Pane,
) -> Option<(pane_grid::Axis, bool)> {
    fn walk(
        node: &pane_grid::Node,
        source: pane_grid::Pane,
        target: pane_grid::Pane,
    ) -> Option<(pane_grid::Axis, bool)> {
        match node {
            pane_grid::Node::Pane(_) => None,
            pane_grid::Node::Split {
                axis,
                a: lhs,
                b: rhs,
                ..
            } => {
                if let (pane_grid::Node::Pane(pa), pane_grid::Node::Pane(pb)) = (&**lhs, &**rhs) {
                    if *pa == source && *pb == target {
                        return Some((*axis, true));
                    }
                    if *pa == target && *pb == source {
                        return Some((*axis, false));
                    }
                }
                walk(lhs, source, target).or_else(|| walk(rhs, source, target))
            }
        }
    }
    if source == target {
        None
    } else {
        walk(layout, source, target)
    }
}

// Overlay widget stacked above the PaneGrid that draws a single drop-preview
// rectangle. Always present in the view (so the widget tree shape doesn't
// shift between drags, which would reset terminal_box's tracked modifier
// state); renders nothing when `drag` is None.
pub struct PaneDropPreview<'a> {
    layout: &'a pane_grid::Node,
    drag: Option<(pane_grid::Pane, pane_grid::Pane, DropRegion)>,
    // Must match the values the host configures on its PaneGrid or the
    // computed bounds drift from the pane bounds the user sees.
    spacing: f32,
    min_size: f32,
}

impl<'a> PaneDropPreview<'a> {
    pub fn new(
        layout: &'a pane_grid::Node,
        drag: Option<(pane_grid::Pane, pane_grid::Pane, DropRegion)>,
    ) -> Self {
        Self {
            layout,
            drag,
            spacing: 0.0,
            min_size: 50.0,
        }
    }

    // Returns the preview rectangle in grid-local coordinates; the caller
    // offsets by its own layout origin to produce screen coordinates.
    fn preview_rect(&self, total_size: Size) -> Option<Rectangle> {
        let (source, hovered, region) = self.drag?;
        let regions = self
            .layout
            .pane_regions(self.spacing, self.min_size, total_size);

        let source_bounds = *regions.get(&source)?;

        // Hovering the source pane = no-op-on-release indicator.
        if source == hovered {
            return Some(source_bounds);
        }

        let target_bounds = *regions.get(&hovered)?;

        // Center on a different pane is a swap; source lands on target.
        if region == DropRegion::Center {
            return Some(target_bounds);
        }

        // Sibling Edge drops reflow the parent split's whole area, so the
        // destination is half of (source + target), not half of target.
        let parent_bounds = if sibling_split_info(self.layout, source, hovered).is_some() {
            source_bounds.union(&target_bounds)
        } else {
            target_bounds
        };

        Some(region.preview_bounds(parent_bounds))
    }
}

impl<Message> Widget<Message, Theme, Renderer> for PaneDropPreview<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        // Pass through; the pane_grid underneath drives the cursor.
        mouse::Interaction::None
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(local) = self.preview_rect(bounds.size()) else {
            return;
        };
        let rect = Rectangle {
            x: bounds.x + local.x,
            y: bounds.y + local.y,
            width: local.width,
            height: local.height,
        };
        let accent: Color = theme.cosmic().accent.base.into();
        let fill = Color { a: 0.25, ..accent };
        let border_color = Color { a: 0.9, ..accent };
        renderer.fill_quad(
            Quad {
                bounds: rect,
                border: Border {
                    radius: [0.0; 4].into(),
                    width: 2.0,
                    color: border_color,
                },
                snap: true,
                ..Default::default()
            },
            fill,
        );
    }
}

impl<'a, Message: 'a> From<PaneDropPreview<'a>> for Element<'a, Message, Theme, Renderer> {
    fn from(preview: PaneDropPreview<'a>) -> Self {
        Self::new(preview)
    }
}

// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget::pane_grid::{self, Axis, Node, Pane};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

use crate::config::ProfileId;

pub const SESSION_VERSION: u64 = 1;
pub const SESSION_KEY: &str = "terminal-session";

/// Persistent description of the terminal window layout.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Session {
    pub layout: Option<SessionLayout>,
    pub focused: usize,
    pub maximized: Option<usize>,
}

impl Session {
    pub fn is_restorable(&self) -> bool {
        self.layout
            .as_ref()
            .is_some_and(SessionLayout::is_restorable)
    }

    pub fn into_pane_configuration(
        self,
    ) -> Option<(pane_grid::Configuration<PaneSession>, usize, Option<usize>)> {
        let layout = self.layout?;
        if !layout.is_restorable() {
            return None;
        }

        Some((layout.into_configuration(), self.focused, self.maximized))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLayout {
    Split {
        axis: SessionAxis,
        ratio: f32,
        first: Box<SessionLayout>,
        second: Box<SessionLayout>,
    },
    Pane(PaneSession),
}

impl SessionLayout {
    pub fn is_restorable(&self) -> bool {
        match self {
            Self::Split { first, second, .. } => first.is_restorable() && second.is_restorable(),
            Self::Pane(pane) => !pane.tabs.is_empty(),
        }
    }

    #[cfg(test)]
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Split { first, second, .. } => first.pane_count() + second.pane_count(),
            Self::Pane(_) => 1,
        }
    }

    fn into_configuration(self) -> pane_grid::Configuration<PaneSession> {
        match self {
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => pane_grid::Configuration::Split {
                axis: axis.into(),
                ratio: safe_ratio(ratio),
                a: Box::new(first.into_configuration()),
                b: Box::new(second.into_configuration()),
            },
            Self::Pane(pane) => pane_grid::Configuration::Pane(pane),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAxis {
    Horizontal,
    Vertical,
}

impl From<Axis> for SessionAxis {
    fn from(axis: Axis) -> Self {
        match axis {
            Axis::Horizontal => Self::Horizontal,
            Axis::Vertical => Self::Vertical,
        }
    }
}

impl From<SessionAxis> for Axis {
    fn from(axis: SessionAxis) -> Self {
        match axis {
            SessionAxis::Horizontal => Self::Horizontal,
            SessionAxis::Vertical => Self::Vertical,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct PaneSession {
    pub active_tab: usize,
    pub tabs: Vec<TabSession>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct TabSession {
    pub profile_id: Option<ProfileId>,
    pub working_directory: Option<PathBuf>,
}

pub fn layout_from_node(node: &Node, panes: &HashMap<Pane, PaneSession>) -> Option<SessionLayout> {
    match node {
        Node::Split {
            axis, ratio, a, b, ..
        } => Some(SessionLayout::Split {
            axis: (*axis).into(),
            ratio: safe_ratio(*ratio),
            first: Box::new(layout_from_node(a, panes)?),
            second: Box::new(layout_from_node(b, panes)?),
        }),
        Node::Pane(pane) => panes.get(pane).cloned().map(SessionLayout::Pane),
    }
}

fn safe_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session() -> Session {
        Session {
            layout: Some(SessionLayout::Split {
                axis: SessionAxis::Vertical,
                ratio: 0.35,
                first: Box::new(SessionLayout::Pane(PaneSession {
                    active_tab: 1,
                    tabs: vec![
                        TabSession {
                            profile_id: Some(ProfileId(3)),
                            working_directory: Some(PathBuf::from("/tmp")),
                        },
                        TabSession::default(),
                    ],
                })),
                second: Box::new(SessionLayout::Pane(PaneSession {
                    active_tab: 0,
                    tabs: vec![TabSession::default()],
                })),
            }),
            focused: 1,
            maximized: None,
        }
    }

    #[test]
    fn session_round_trips_through_ron() {
        let session = sample_session();
        let serialized =
            ron::ser::to_string_pretty(&session, ron::ser::PrettyConfig::new()).unwrap();
        let deserialized: Session = ron::from_str(&serialized).unwrap();
        assert_eq!(session, deserialized);
    }

    #[test]
    fn empty_session_is_not_restorable() {
        assert!(!Session::default().is_restorable());
        assert!(sample_session().is_restorable());
    }

    #[test]
    fn pane_without_tabs_is_not_restorable() {
        let session = Session {
            layout: Some(SessionLayout::Pane(PaneSession::default())),
            ..Session::default()
        };
        assert!(!session.is_restorable());
    }

    #[test]
    fn layout_round_trips_through_pane_grid() {
        let layout = sample_session().layout.unwrap();
        let state = pane_grid::State::with_configuration(layout.clone().into_configuration());
        let panes: HashMap<_, _> = state
            .panes
            .iter()
            .map(|(pane, session)| (*pane, session.clone()))
            .collect();
        let rebuilt = layout_from_node(state.layout(), &panes).unwrap();
        assert_eq!(layout, rebuilt);
    }

    #[test]
    fn layout_counts_panes() {
        assert_eq!(sample_session().layout.unwrap().pane_count(), 2);
    }

    #[test]
    fn invalid_ratio_is_replaced() {
        assert_eq!(safe_ratio(f32::NAN), 0.5);
        assert_eq!(safe_ratio(-1.0), 0.0);
        assert_eq!(safe_ratio(2.0), 1.0);
    }
}

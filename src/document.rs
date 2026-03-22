use serde::{Deserialize, Serialize};

use crate::drawing::Element;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Document {
    pub version: u32,
    pub name: String,
    pub canvas_view: CanvasViewState,
    pub elements: Vec<Element>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CanvasViewState {
    pub offset: [f32; 2],
    pub zoom: f32,
}

impl Document {
    pub fn new() -> Self {
        Self {
            version: SCHEMA_VERSION,
            name: "Untitled".to_string(),
            canvas_view: CanvasViewState {
                offset: [0.0, 0.0],
                zoom: 1.0,
            },
            elements: Vec::new(),
        }
    }

    pub fn from_state(
        elements: &[Element],
        offset: [f32; 2],
        zoom: f32,
        name: Option<&str>,
    ) -> Self {
        Self {
            version: SCHEMA_VERSION,
            name: name.unwrap_or("Untitled").to_string(),
            canvas_view: CanvasViewState { offset, zoom },
            elements: elements.to_vec(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{DrawingElement, Element, ElementId};
    use crate::rough::RoughOptions;

    #[test]
    fn test_empty_document_roundtrip() {
        let doc = Document::new();
        let json = doc.to_json().unwrap();
        let doc2 = Document::from_json(&json).unwrap();
        assert_eq!(doc2.version, SCHEMA_VERSION);
        assert_eq!(doc2.name, "Untitled");
        assert!(doc2.elements.is_empty());
    }

    #[test]
    fn test_document_with_all_element_types() {
        let elements = vec![
            Element {
                id: ElementId(1),
                group_id: None,
                shape: DrawingElement::Stroke {
                    points: vec![[0.0, 0.0], [10.0, 10.0], [20.0, 5.0]],
                    color: [0.0, 0.0, 0.0, 1.0],
                    width: 2.0,
                },
            },
            Element {
                id: ElementId(2),
                group_id: None,
                shape: DrawingElement::Line {
                    start: [0.0, 0.0],
                    end: [100.0, 100.0],
                    color: [1.0, 0.0, 0.0, 1.0],
                    width: 3.0,
                    rough_style: Some(RoughOptions {
                        roughness: 1.0,
                        bowing: 1.0,
                        stroke_width: 2.0,
                        seed: Some(42),
                        ..RoughOptions::default()
                    }),
                },
            },
            Element {
                id: ElementId(3),
                group_id: None,
                shape: DrawingElement::Rectangle {
                    position: [50.0, 50.0],
                    size: [200.0, 100.0],
                    color: [0.0, 1.0, 0.0, 1.0],
                    fill: true,
                    stroke_width: 1.5,
                    rough_style: None,
                },
            },
            Element {
                id: ElementId(4),
                group_id: None,
                shape: DrawingElement::Circle {
                    center: [150.0, 150.0],
                    radius: 75.0,
                    color: [0.0, 0.0, 1.0, 1.0],
                    fill: false,
                    stroke_width: 2.0,
                    rough_style: None,
                },
            },
            Element {
                id: ElementId(5),
                group_id: None,
                shape: DrawingElement::Diamond {
                    position: [300.0, 100.0],
                    size: [80.0, 60.0],
                    color: [1.0, 1.0, 0.0, 1.0],
                    fill: false,
                    stroke_width: 2.5,
                    rough_style: None,
                },
            },
            Element {
                id: ElementId(6),
                group_id: None,
                shape: DrawingElement::Arrow {
                    start: [10.0, 10.0],
                    end: [200.0, 200.0],
                    color: [0.5, 0.5, 0.5, 1.0],
                    width: 2.0,
                    rough_style: None,
                },
            },
            Element {
                id: ElementId(7),
                group_id: None,
                shape: DrawingElement::Text {
                    position: [100.0, 300.0],
                    content: "Hello, wcanvas!".to_string(),
                    color: [0.0, 0.0, 0.0, 1.0],
                    size: 32.0,
                },
            },
            Element {
                id: ElementId(8),
                group_id: None,
                shape: DrawingElement::TextBox {
                    id: 1,
                    pos: [400.0, 200.0],
                    size: [150.0, 50.0],
                    content: "Editable text".to_string(),
                    color: [0.2, 0.2, 0.2, 1.0],
                    font_size: 16.0,
                    state: crate::drawing::BoxState::Idle,
                },
            },
        ];

        let doc = Document::from_state(&elements, [10.0, 20.0], 1.5, Some("Test Drawing"));
        let json = doc.to_json().unwrap();
        let doc2 = Document::from_json(&json).unwrap();

        assert_eq!(doc2.version, SCHEMA_VERSION);
        assert_eq!(doc2.name, "Test Drawing");
        assert_eq!(doc2.elements.len(), 8);
        assert_eq!(doc2.canvas_view.offset, [10.0, 20.0]);
        assert!((doc2.canvas_view.zoom - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_schema_version_present() {
        let doc = Document::new();
        let json = doc.to_json().unwrap();
        assert!(json.contains("\"version\": 1"));
    }
}

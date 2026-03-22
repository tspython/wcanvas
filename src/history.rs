use crate::drawing::{Element, ElementId};

#[derive(Debug, Clone)]
pub enum Action {
    Add {
        elements: Vec<(usize, Element)>,
    },
    Remove {
        elements: Vec<(usize, Element)>,
    },
    Move {
        before: Vec<Element>,
        after: Vec<Element>,
    },
    ModifyProperty {
        before: Vec<Element>,
        after: Vec<Element>,
    },
    Reorder {
        before: Vec<ElementId>,
        after: Vec<ElementId>,
    },
    Batch(Vec<Action>),
}

#[derive(Debug, Default)]
pub struct History {
    pub undo_stack: Vec<Action>,
    pub redo_stack: Vec<Action>,
}

impl History {
    pub fn push(&mut self, action: Action) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

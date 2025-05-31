pub use derive_tree_sitter_macros::*;
use std::ops::Range;

pub struct Node<'a, 'tree> {
    source: &'tree str,
    cursor: Option<&'a mut tree_sitter::TreeCursor<'tree>>,
    errors: Option<&'a mut Vec<Range<usize>>>,
    inner: tree_sitter::Node<'tree>,
}

impl<'a, 'tree> Node<'a, 'tree> {
    fn root(
        source: &'tree str,
        cursor: &'a mut tree_sitter::TreeCursor<'tree>,
        errors: &'a mut Vec<Range<usize>>,
        inner: tree_sitter::Node<'tree>,
    ) -> Self {
        Node {
            source,
            cursor: Some(cursor),
            errors: Some(errors),
            inner,
        }
    }

    fn with<T>(&mut self, child: tree_sitter::Node<'tree>, f: impl FnOnce(&mut Self) -> T) -> T {
        let mut node = Node {
            source: self.source,
            cursor: self.cursor.take(),
            errors: self.errors.take(),
            inner: child,
        };

        let result = f(&mut node);

        self.cursor = node.cursor;
        self.errors = node.errors;

        result
    }
}

pub trait FromNode {
    fn from_node(node: &mut Node<'_, '_>) -> Self;
}

impl<T: FromNode> FromNode for Box<T> {
    fn from_node(node: &mut Node<'_, '_>) -> Self {
        Box::new(T::from_node(node))
    }
}

impl Node<'_, '_> {
    pub fn kind(&self) -> &str {
        self.inner.kind()
    }

    #[track_caller]
    pub fn span(&self) -> Range<usize> {
        self.inner.byte_range()
    }

    #[track_caller]
    pub fn slice(&self) -> String {
        self.inner
            .utf8_text(self.source.as_bytes())
            .unwrap_or_else(|_| unreachable!("source is always UTF-8"))
            .to_string()
    }

    #[track_caller]
    pub fn try_child<T: FromNode>(&mut self, rule: &str) -> Option<T> {
        self.inner
            .child_by_field_name(rule)
            .map(|child| self.with(child, |node| T::from_node(node)))
    }

    #[track_caller]
    pub fn child<T: FromNode>(&mut self, rule: &str) -> T {
        self.try_child(rule)
            .unwrap_or_else(|| panic!("missing child '{rule}'"))
    }

    #[track_caller]
    pub fn children<T: FromNode>(&mut self, rule: &str) -> Vec<T> {
        let children = {
            self.inner
                .children_by_field_name(rule, self.cursor.as_mut().unwrap())
                .collect::<Vec<_>>()
        };

        children
            .into_iter()
            .map(|child| self.with(child, |node| T::from_node(node)))
            .collect()
    }

    pub fn has_child(&self, rule: &str) -> bool {
        self.inner.child_by_field_name(rule).is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Result<T> {
    pub value: T,
    pub errors: Vec<Range<usize>>,
}

pub fn parse<T: FromNode>(source: &str, language: impl Into<tree_sitter::Language>) -> Result<T> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language.into())
        .expect("failed to set language");

    let tree = parser.parse(source, None).expect("failed to produce tree");

    let mut cursor = tree.walk();
    let mut errors = Vec::new();

    let value = T::from_node(&mut Node::root(
        source,
        &mut cursor,
        &mut errors,
        tree.root_node(),
    ));

    Result { value, errors }
}

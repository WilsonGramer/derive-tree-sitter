#[cfg(target_arch = "wasm32")]
extern crate tree_sitter_c2rust as tree_sitter;

pub use derive_tree_sitter_macros::*;
use std::ops::Range;

pub struct Node<'a, 'tree> {
    source: &'tree str,
    cursor: Option<&'a mut tree_sitter::TreeCursor<'tree>>,
    inner: tree_sitter::Node<'tree>,
}

impl<'a, 'tree> Node<'a, 'tree> {
    fn root(
        source: &'tree str,
        cursor: &'a mut tree_sitter::TreeCursor<'tree>,
        inner: tree_sitter::Node<'tree>,
    ) -> Self {
        Node {
            source,
            cursor: Some(cursor),
            inner,
        }
    }

    fn with<T>(
        &mut self,
        child: tree_sitter::Node<'tree>,
        f: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let mut node = Node {
            source: self.source,
            cursor: self.cursor.take(),
            inner: child,
        };

        let result = f(&mut node);

        self.cursor = node.cursor;

        result
    }
}

pub trait FromNode: Sized {
    fn from_node(node: &mut Node<'_, '_>) -> Result<Self>;
}

impl<T: FromNode> FromNode for Box<T> {
    fn from_node(node: &mut Node<'_, '_>) -> Result<Self> {
        Ok(Box::new(T::from_node(node)?))
    }
}

impl Node<'_, '_> {
    pub fn kind(&self) -> &str {
        self.inner.kind()
    }

    #[track_caller]
    pub fn range(&self) -> Range<usize> {
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
    pub fn try_child<T: FromNode>(&mut self, rule: &str) -> Result<Option<T>> {
        self.inner
            .child_by_field_name(rule)
            .map(|child| self.with(child, T::from_node))
            .transpose()
    }

    #[track_caller]
    pub fn child<T: FromNode>(&mut self, rule: &str) -> Result<T> {
        let child = self
            .inner
            .children_by_field_name(rule, self.cursor.as_mut().unwrap())
            .find(|child| child.is_named())
            .unwrap_or_else(|| panic!("missing child '{rule}'"));

        self.with(child, T::from_node)
    }

    #[track_caller]
    pub fn children<T: FromNode>(&mut self, rule: &str) -> Result<Vec<T>> {
        let children = self
            .inner
            .children_by_field_name(rule, self.cursor.as_mut().unwrap())
            .collect::<Vec<_>>();

        children
            .into_iter()
            .filter(|child| child.is_named())
            .map(|child| self.with(child, T::from_node))
            .collect()
    }

    pub fn has_child(&self, rule: &str) -> bool {
        self.inner.child_by_field_name(rule).is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Error {
    pub range: Range<usize>,
    pub kind: ErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Node,
    Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn parse<T: FromNode>(source: &str, language: impl Into<tree_sitter::Language>) -> Result<T> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language.into())
        .expect("failed to set language");

    let tree = parser.parse(source, None).expect("failed to produce tree");

    check_for_errors(&mut tree.walk())?;

    T::from_node(&mut Node::root(source, &mut tree.walk(), tree.root_node()))
}

fn check_for_errors(cursor: &mut tree_sitter::TreeCursor<'_>) -> Result<()> {
    if cursor.node().is_error() {
        return Err(Error {
            range: cursor.node().byte_range(),
            kind: ErrorKind::Node,
        });
    }

    while cursor.goto_next_sibling() {
        check_for_errors(cursor)?;

        if cursor.goto_first_child() {
            check_for_errors(cursor)?;
            cursor.goto_parent();
        }
    }

    Ok(())
}

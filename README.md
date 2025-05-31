# derive-tree-sitter

Convert [tree-sitter](https://github.com/tree-sitter/tree-sitter) parse trees to Rust data structures using a derive macro.

This project is a work-in-progress and doesn't handle errors yet.

## Example

Add a language to your Cargo.toml:

```toml
[dependencies]
tree-sitter-mylanguage = "*"
```

Define your data structures and add `#[derive(FromNode)]`:

```rust
use derive_tree_sitter::FromNode;

#[derive(Debug, PartialEq, Eq, FromNode)]
struct SourceFile {
    variables: Vec<VariableAssignment>,
}

#[derive(Debug, PartialEq, Eq, FromNode)]
struct VariableAssignment {
    variable: String,
    value: Expression,
}

#[derive(Debug, PartialEq, Eq, FromNode)]
enum Expression {
    #[tree_sitter(rule = "int_expression")]
    Int(IntExpression),

    #[tree_sitter(rule = "variable_expression")]
    Variable(VariableExpression),
}

#[derive(Debug, PartialEq, Eq, FromNode)]
struct IntExpression {
    raw: String,
}

#[derive(Debug, PartialEq, Eq, FromNode)]
struct VariableExpression {
    raw: String,
}
```

Then, use `parse` to get an AST:

```rust
let source = "let x = 1";

let result: derive_tree_sitter::Result<SourceFile> =
    derive_tree_sitter::parse(source, tree_sitter_mylanguage::LANGUAGE);

assert_eq!(result, SourceFile {
    variables: vec![VariableAssignment {
        variable: String::from("x"),
        value: Expression::Int(IntExpression {
            raw: String::from("1"),
        }),
    }],
});
```

## Supported types

In addition to any type implementing `FromNode`, you can use:

-   `Range<usize>` to get the current span
-   `String` to get the current slice of the source code
-   `Vec<T>` to aggregate multiple fields
-   `Option<T>` for optional fields
-   `bool` to check if a field is present

//! Aegis AST - Abstract Syntax Tree
//!
//! Internal representation of parsed SQL statements. Provides a clean,
//! typed representation that is easier to analyze and optimize than
//! the raw parser output.
//!
//! Key Features:
//! - Typed expression trees
//! - Statement variants for DML and DDL
//! - Support for subqueries and CTEs
//! - Extensible for custom Aegis features
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_common::DataType;
use serde::{Deserialize, Serialize};

// =============================================================================
// Statements
// =============================================================================

/// Top-level SQL statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    CreateTable(CreateTableStatement),
    DropTable(DropTableStatement),
    AlterTable(AlterTableStatement),
    CreateIndex(CreateIndexStatement),
    DropIndex(DropIndexStatement),
    Begin,
    Commit,
    Rollback,
}

// =============================================================================
// SELECT Statement
// =============================================================================

/// A SELECT query.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub distinct: bool,
    pub columns: Vec<SelectColumn>,
    pub from: Option<FromClause>,
    pub where_clause: Option<Expression>,
    pub group_by: Vec<Expression>,
    pub having: Option<Expression>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

impl Default for SelectStatement {
    fn default() -> Self {
        Self {
            distinct: false,
            columns: Vec::new(),
            from: None,
            where_clause: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        }
    }
}

/// A column in SELECT list.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectColumn {
    AllColumns,
    TableAllColumns(String),
    Expression { expr: Expression, alias: Option<String> },
}

/// FROM clause.
#[derive(Debug, Clone, PartialEq)]
pub struct FromClause {
    pub source: TableReference,
    pub joins: Vec<JoinClause>,
}

/// Reference to a table or subquery.
#[derive(Debug, Clone, PartialEq)]
pub enum TableReference {
    Table { name: String, alias: Option<String> },
    Subquery { query: Box<SelectStatement>, alias: String },
}

/// JOIN clause.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: TableReference,
    pub condition: Option<Expression>,
}

/// Type of JOIN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

/// ORDER BY item.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    pub expression: Expression,
    pub ascending: bool,
    pub nulls_first: Option<bool>,
}

// =============================================================================
// INSERT Statement
// =============================================================================

/// An INSERT statement.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStatement {
    pub table: String,
    pub columns: Option<Vec<String>>,
    pub source: InsertSource,
}

/// Source for INSERT data.
#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    Values(Vec<Vec<Expression>>),
    Query(Box<SelectStatement>),
}

// =============================================================================
// UPDATE Statement
// =============================================================================

/// An UPDATE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStatement {
    pub table: String,
    pub assignments: Vec<Assignment>,
    pub where_clause: Option<Expression>,
}

/// Column assignment.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: String,
    pub value: Expression,
}

// =============================================================================
// DELETE Statement
// =============================================================================

/// A DELETE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStatement {
    pub table: String,
    pub where_clause: Option<Expression>,
}

// =============================================================================
// DDL Statements
// =============================================================================

/// CREATE TABLE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStatement {
    pub name: String,
    pub columns: Vec<ColumnDefinition>,
    pub constraints: Vec<TableConstraint>,
    pub if_not_exists: bool,
}

/// Column definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<Expression>,
    pub constraints: Vec<ColumnConstraint>,
}

/// Column-level constraint.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    PrimaryKey,
    Unique,
    NotNull,
    Check(Expression),
    References { table: String, column: String },
}

/// Table-level constraint.
#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    PrimaryKey { columns: Vec<String> },
    Unique { columns: Vec<String> },
    ForeignKey {
        columns: Vec<String>,
        ref_table: String,
        ref_columns: Vec<String>,
    },
    Check { expression: Expression },
}

/// DROP TABLE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct DropTableStatement {
    pub name: String,
    pub if_exists: bool,
}

/// ALTER TABLE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct AlterTableStatement {
    pub name: String,
    pub operations: Vec<AlterTableOperation>,
}

/// ALTER TABLE operation.
#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableOperation {
    AddColumn(ColumnDefinition),
    DropColumn { name: String, if_exists: bool },
    RenameColumn { old_name: String, new_name: String },
    AlterColumn { name: String, data_type: Option<DataType>, set_not_null: Option<bool>, set_default: Option<Option<Expression>> },
    RenameTable { new_name: String },
    AddConstraint(TableConstraint),
    DropConstraint { name: String },
}

/// CREATE INDEX statement.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexStatement {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub if_not_exists: bool,
}

/// DROP INDEX statement.
#[derive(Debug, Clone, PartialEq)]
pub struct DropIndexStatement {
    pub name: String,
    pub if_exists: bool,
}

// =============================================================================
// Expressions
// =============================================================================

/// An expression in SQL.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Column(ColumnRef),
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expression>,
    },
    Function {
        name: String,
        args: Vec<Expression>,
        distinct: bool,
    },
    Case {
        operand: Option<Box<Expression>>,
        conditions: Vec<(Expression, Expression)>,
        else_result: Option<Box<Expression>>,
    },
    Cast {
        expr: Box<Expression>,
        data_type: DataType,
    },
    InList {
        expr: Box<Expression>,
        list: Vec<Expression>,
        negated: bool,
    },
    InSubquery {
        expr: Box<Expression>,
        subquery: Box<SelectStatement>,
        negated: bool,
    },
    Between {
        expr: Box<Expression>,
        low: Box<Expression>,
        high: Box<Expression>,
        negated: bool,
    },
    Like {
        expr: Box<Expression>,
        pattern: Box<Expression>,
        negated: bool,
    },
    IsNull {
        expr: Box<Expression>,
        negated: bool,
    },
    Exists {
        subquery: Box<SelectStatement>,
        negated: bool,
    },
    Subquery(Box<SelectStatement>),
    Placeholder(usize),
}

/// Literal value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

/// Column reference.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnRef {
    pub table: Option<String>,
    pub column: String,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,

    // Comparison
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    // Logical
    And,
    Or,

    // String
    Concat,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Negative,
    Positive,
}

// =============================================================================
// Utility Implementations
// =============================================================================

impl Expression {
    /// Create a column reference expression.
    pub fn column(name: &str) -> Self {
        Expression::Column(ColumnRef {
            table: None,
            column: name.to_string(),
        })
    }

    /// Create a qualified column reference.
    pub fn qualified_column(table: &str, column: &str) -> Self {
        Expression::Column(ColumnRef {
            table: Some(table.to_string()),
            column: column.to_string(),
        })
    }

    /// Create an integer literal.
    pub fn int(value: i64) -> Self {
        Expression::Literal(Literal::Integer(value))
    }

    /// Create a string literal.
    pub fn string(value: &str) -> Self {
        Expression::Literal(Literal::String(value.to_string()))
    }

    /// Create a boolean literal.
    pub fn boolean(value: bool) -> Self {
        Expression::Literal(Literal::Boolean(value))
    }

    /// Create a NULL literal.
    pub fn null() -> Self {
        Expression::Literal(Literal::Null)
    }
}

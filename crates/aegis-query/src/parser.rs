//! Aegis Parser - SQL Parser
//!
//! Wraps sqlparser-rs to parse SQL statements and convert them to the
//! internal Aegis AST representation. Handles dialect differences and
//! provides friendly error messages.
//!
//! Key Features:
//! - Full ANSI SQL support via sqlparser-rs
//! - Conversion to typed Aegis AST
//! - Detailed parse error reporting
//! - Support for multiple SQL dialects
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::ast::*;
use aegis_common::{AegisError, DataType, Result};
use sqlparser::ast as sp;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser as SqlParser;

// =============================================================================
// Parser
// =============================================================================

/// SQL parser wrapping sqlparser-rs.
pub struct Parser {
    dialect: GenericDialect,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            dialect: GenericDialect {},
        }
    }

    /// Parse a SQL string into statements.
    pub fn parse(&self, sql: &str) -> Result<Vec<Statement>> {
        let ast = SqlParser::parse_sql(&self.dialect, sql)
            .map_err(|e| AegisError::Parse(e.to_string()))?;

        ast.into_iter().map(|stmt| self.convert_statement(stmt)).collect()
    }

    /// Parse a single statement.
    pub fn parse_single(&self, sql: &str) -> Result<Statement> {
        let statements = self.parse(sql)?;
        if statements.len() != 1 {
            return Err(AegisError::Parse(format!(
                "Expected 1 statement, got {}",
                statements.len()
            )));
        }
        // Safe to use expect here: we verified statements.len() == 1 above
        Ok(statements.into_iter().next().expect("statements verified to have exactly 1 element"))
    }

    fn convert_statement(&self, stmt: sp::Statement) -> Result<Statement> {
        match stmt {
            sp::Statement::Query(query) => {
                let select = self.convert_query(*query)?;
                Ok(Statement::Select(select))
            }
            sp::Statement::Insert(insert) => {
                let insert_stmt = self.convert_insert(insert)?;
                Ok(Statement::Insert(insert_stmt))
            }
            sp::Statement::Update { table, assignments, from: _, selection, returning: _, .. } => {
                let update_stmt = self.convert_update(table, assignments, selection)?;
                Ok(Statement::Update(update_stmt))
            }
            sp::Statement::Delete(delete) => {
                let delete_stmt = self.convert_delete(delete)?;
                Ok(Statement::Delete(delete_stmt))
            }
            sp::Statement::CreateTable(create) => {
                let create_stmt = self.convert_create_table(create)?;
                Ok(Statement::CreateTable(create_stmt))
            }
            sp::Statement::Drop { object_type, if_exists, names, .. } => {
                self.convert_drop(object_type, if_exists, names)
            }
            sp::Statement::CreateIndex(create) => {
                let create_stmt = self.convert_create_index(create)?;
                Ok(Statement::CreateIndex(create_stmt))
            }
            sp::Statement::AlterTable { name, operations, .. } => {
                let alter_stmt = self.convert_alter_table(name, operations)?;
                Ok(Statement::AlterTable(alter_stmt))
            }
            sp::Statement::StartTransaction { .. } => Ok(Statement::Begin),
            sp::Statement::Commit { .. } => Ok(Statement::Commit),
            sp::Statement::Rollback { .. } => Ok(Statement::Rollback),
            _ => Err(AegisError::Parse(format!(
                "Unsupported statement type: {:?}",
                stmt
            ))),
        }
    }

    fn convert_query(&self, query: sp::Query) -> Result<SelectStatement> {
        let body = match *query.body {
            sp::SetExpr::Select(select) => select,
            _ => return Err(AegisError::Parse("Unsupported query type".to_string())),
        };

        let columns = body
            .projection
            .into_iter()
            .map(|item| self.convert_select_item(item))
            .collect::<Result<Vec<_>>>()?;

        let from = if !body.from.is_empty() {
            Some(self.convert_from(&body.from)?)
        } else {
            None
        };

        let where_clause = body
            .selection
            .map(|expr| self.convert_expr(expr))
            .transpose()?;

        let group_by = match body.group_by {
            sp::GroupByExpr::Expressions(exprs, _) => exprs
                .into_iter()
                .map(|e| self.convert_expr(e))
                .collect::<Result<Vec<_>>>()?,
            sp::GroupByExpr::All(_) => Vec::new(),
        };

        let having = body.having.map(|e| self.convert_expr(e)).transpose()?;

        let order_by = query
            .order_by
            .map(|ob| {
                ob.exprs
                    .into_iter()
                    .map(|item| self.convert_order_by_item(item))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();

        let limit = query
            .limit
            .map(|e| self.extract_limit(e))
            .transpose()?;

        let offset = query
            .offset
            .map(|o| self.extract_limit(o.value))
            .transpose()?;

        Ok(SelectStatement {
            distinct: body.distinct.is_some(),
            columns,
            from,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
        })
    }

    fn convert_select_item(&self, item: sp::SelectItem) -> Result<SelectColumn> {
        match item {
            sp::SelectItem::UnnamedExpr(expr) => Ok(SelectColumn::Expression {
                expr: self.convert_expr(expr)?,
                alias: None,
            }),
            sp::SelectItem::ExprWithAlias { expr, alias } => Ok(SelectColumn::Expression {
                expr: self.convert_expr(expr)?,
                alias: Some(alias.value),
            }),
            sp::SelectItem::Wildcard(_) => Ok(SelectColumn::AllColumns),
            sp::SelectItem::QualifiedWildcard(name, _) => {
                Ok(SelectColumn::TableAllColumns(name.to_string()))
            }
        }
    }

    fn convert_from(&self, from: &[sp::TableWithJoins]) -> Result<FromClause> {
        let first = from.first().ok_or_else(|| AegisError::Parse("Empty FROM".to_string()))?;

        let source = self.convert_table_factor(&first.relation)?;

        let mut joins = Vec::new();
        for join in &first.joins {
            joins.push(self.convert_join(join)?);
        }

        Ok(FromClause { source, joins })
    }

    fn convert_table_factor(&self, factor: &sp::TableFactor) -> Result<TableReference> {
        match factor {
            sp::TableFactor::Table { name, alias, .. } => Ok(TableReference::Table {
                name: name.to_string(),
                alias: alias.as_ref().map(|a| a.name.value.clone()),
            }),
            sp::TableFactor::Derived { subquery, alias, .. } => {
                let alias_name = alias
                    .as_ref()
                    .map(|a| a.name.value.clone())
                    .ok_or_else(|| AegisError::Parse("Subquery requires alias".to_string()))?;
                Ok(TableReference::Subquery {
                    query: Box::new(self.convert_query(*subquery.clone())?),
                    alias: alias_name,
                })
            }
            _ => Err(AegisError::Parse("Unsupported table factor".to_string())),
        }
    }

    fn convert_join(&self, join: &sp::Join) -> Result<JoinClause> {
        let join_type = match &join.join_operator {
            sp::JoinOperator::Inner(_) => JoinType::Inner,
            sp::JoinOperator::LeftOuter(_) => JoinType::Left,
            sp::JoinOperator::RightOuter(_) => JoinType::Right,
            sp::JoinOperator::FullOuter(_) => JoinType::Full,
            sp::JoinOperator::CrossJoin => JoinType::Cross,
            _ => return Err(AegisError::Parse("Unsupported join type".to_string())),
        };

        let condition = match &join.join_operator {
            sp::JoinOperator::Inner(c)
            | sp::JoinOperator::LeftOuter(c)
            | sp::JoinOperator::RightOuter(c)
            | sp::JoinOperator::FullOuter(c) => match c {
                sp::JoinConstraint::On(expr) => Some(self.convert_expr(expr.clone())?),
                sp::JoinConstraint::None => None,
                _ => return Err(AegisError::Parse("Unsupported join constraint".to_string())),
            },
            sp::JoinOperator::CrossJoin => None,
            _ => None,
        };

        Ok(JoinClause {
            join_type,
            table: self.convert_table_factor(&join.relation)?,
            condition,
        })
    }

    fn convert_order_by_item(&self, item: sp::OrderByExpr) -> Result<OrderByItem> {
        Ok(OrderByItem {
            expression: self.convert_expr(item.expr)?,
            ascending: item.asc.unwrap_or(true),
            nulls_first: item.nulls_first,
        })
    }

    fn convert_expr(&self, expr: sp::Expr) -> Result<Expression> {
        match expr {
            sp::Expr::Identifier(ident) => Ok(Expression::Column(ColumnRef {
                table: None,
                column: ident.value,
            })),
            sp::Expr::CompoundIdentifier(idents) => {
                if idents.len() == 2 {
                    Ok(Expression::Column(ColumnRef {
                        table: Some(idents[0].value.clone()),
                        column: idents[1].value.clone(),
                    }))
                } else {
                    Ok(Expression::Column(ColumnRef {
                        table: None,
                        column: idents.last().map(|i| i.value.clone()).unwrap_or_default(),
                    }))
                }
            }
            sp::Expr::Value(value) => self.convert_value(value),
            sp::Expr::BinaryOp { left, op, right } => Ok(Expression::BinaryOp {
                left: Box::new(self.convert_expr(*left)?),
                op: self.convert_binary_op(op)?,
                right: Box::new(self.convert_expr(*right)?),
            }),
            sp::Expr::UnaryOp { op, expr } => Ok(Expression::UnaryOp {
                op: self.convert_unary_op(op)?,
                expr: Box::new(self.convert_expr(*expr)?),
            }),
            sp::Expr::Function(func) => {
                let args = match func.args {
                    sp::FunctionArguments::List(list) => list
                        .args
                        .into_iter()
                        .filter_map(|arg| match arg {
                            sp::FunctionArg::Unnamed(sp::FunctionArgExpr::Expr(e)) => {
                                Some(self.convert_expr(e))
                            }
                            _ => None,
                        })
                        .collect::<Result<Vec<_>>>()?,
                    _ => Vec::new(),
                };

                Ok(Expression::Function {
                    name: func.name.to_string(),
                    args,
                    distinct: false,
                })
            }
            sp::Expr::Nested(expr) => self.convert_expr(*expr),
            sp::Expr::IsNull(expr) => Ok(Expression::IsNull {
                expr: Box::new(self.convert_expr(*expr)?),
                negated: false,
            }),
            sp::Expr::IsNotNull(expr) => Ok(Expression::IsNull {
                expr: Box::new(self.convert_expr(*expr)?),
                negated: true,
            }),
            sp::Expr::InList { expr, list, negated } => Ok(Expression::InList {
                expr: Box::new(self.convert_expr(*expr)?),
                list: list
                    .into_iter()
                    .map(|e| self.convert_expr(e))
                    .collect::<Result<Vec<_>>>()?,
                negated,
            }),
            sp::Expr::Between { expr, low, high, negated } => Ok(Expression::Between {
                expr: Box::new(self.convert_expr(*expr)?),
                low: Box::new(self.convert_expr(*low)?),
                high: Box::new(self.convert_expr(*high)?),
                negated,
            }),
            sp::Expr::Like { expr, pattern, negated, .. } => Ok(Expression::Like {
                expr: Box::new(self.convert_expr(*expr)?),
                pattern: Box::new(self.convert_expr(*pattern)?),
                negated,
            }),
            _ => Err(AegisError::Parse(format!(
                "Unsupported expression: {:?}",
                expr
            ))),
        }
    }

    fn convert_value(&self, value: sp::Value) -> Result<Expression> {
        let literal = match value {
            sp::Value::Null => Literal::Null,
            sp::Value::Boolean(b) => Literal::Boolean(b),
            sp::Value::Number(n, _) => {
                if n.contains('.') {
                    Literal::Float(n.parse().map_err(|_| AegisError::Parse("Invalid float".to_string()))?)
                } else {
                    Literal::Integer(n.parse().map_err(|_| AegisError::Parse("Invalid integer".to_string()))?)
                }
            }
            sp::Value::SingleQuotedString(s) | sp::Value::DoubleQuotedString(s) => Literal::String(s),
            _ => return Err(AegisError::Parse("Unsupported literal value".to_string())),
        };
        Ok(Expression::Literal(literal))
    }

    fn convert_binary_op(&self, op: sp::BinaryOperator) -> Result<BinaryOperator> {
        match op {
            sp::BinaryOperator::Plus => Ok(BinaryOperator::Add),
            sp::BinaryOperator::Minus => Ok(BinaryOperator::Subtract),
            sp::BinaryOperator::Multiply => Ok(BinaryOperator::Multiply),
            sp::BinaryOperator::Divide => Ok(BinaryOperator::Divide),
            sp::BinaryOperator::Modulo => Ok(BinaryOperator::Modulo),
            sp::BinaryOperator::Eq => Ok(BinaryOperator::Equal),
            sp::BinaryOperator::NotEq => Ok(BinaryOperator::NotEqual),
            sp::BinaryOperator::Lt => Ok(BinaryOperator::LessThan),
            sp::BinaryOperator::LtEq => Ok(BinaryOperator::LessThanOrEqual),
            sp::BinaryOperator::Gt => Ok(BinaryOperator::GreaterThan),
            sp::BinaryOperator::GtEq => Ok(BinaryOperator::GreaterThanOrEqual),
            sp::BinaryOperator::And => Ok(BinaryOperator::And),
            sp::BinaryOperator::Or => Ok(BinaryOperator::Or),
            sp::BinaryOperator::StringConcat => Ok(BinaryOperator::Concat),
            _ => Err(AegisError::Parse(format!("Unsupported operator: {:?}", op))),
        }
    }

    fn convert_unary_op(&self, op: sp::UnaryOperator) -> Result<UnaryOperator> {
        match op {
            sp::UnaryOperator::Not => Ok(UnaryOperator::Not),
            sp::UnaryOperator::Minus => Ok(UnaryOperator::Negative),
            sp::UnaryOperator::Plus => Ok(UnaryOperator::Positive),
            _ => Err(AegisError::Parse(format!("Unsupported unary operator: {:?}", op))),
        }
    }

    fn convert_insert(&self, insert: sp::Insert) -> Result<InsertStatement> {
        let table = insert.table_name.to_string();

        let columns = if insert.columns.is_empty() {
            None
        } else {
            Some(insert.columns.into_iter().map(|c| c.value).collect())
        };

        let source = match insert.source {
            Some(query) => match *query.body {
                sp::SetExpr::Values(values) => {
                    let rows = values
                        .rows
                        .into_iter()
                        .map(|row| {
                            row.into_iter()
                                .map(|e| self.convert_expr(e))
                                .collect::<Result<Vec<_>>>()
                        })
                        .collect::<Result<Vec<_>>>()?;
                    InsertSource::Values(rows)
                }
                _ => InsertSource::Query(Box::new(self.convert_query(*query)?)),
            },
            None => return Err(AegisError::Parse("INSERT missing values".to_string())),
        };

        Ok(InsertStatement {
            table,
            columns,
            source,
        })
    }

    fn convert_update(
        &self,
        table: sp::TableWithJoins,
        assignments: Vec<sp::Assignment>,
        selection: Option<sp::Expr>,
    ) -> Result<UpdateStatement> {
        let table_name = match &table.relation {
            sp::TableFactor::Table { name, .. } => name.to_string(),
            _ => return Err(AegisError::Parse("UPDATE requires table name".to_string())),
        };

        let assigns = assignments
            .into_iter()
            .map(|a| {
                let column = match a.target {
                    sp::AssignmentTarget::ColumnName(names) => {
                        names.0.into_iter().map(|i| i.value).collect::<Vec<_>>().join(".")
                    }
                    sp::AssignmentTarget::Tuple(cols) => {
                        cols.into_iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                };
                Ok(Assignment {
                    column,
                    value: self.convert_expr(a.value)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let where_clause = selection.map(|e| self.convert_expr(e)).transpose()?;

        Ok(UpdateStatement {
            table: table_name,
            assignments: assigns,
            where_clause,
        })
    }

    fn convert_delete(&self, delete: sp::Delete) -> Result<DeleteStatement> {
        let table = match delete.from {
            sp::FromTable::WithFromKeyword(tables) => {
                tables.first()
                    .map(|t| match &t.relation {
                        sp::TableFactor::Table { name, .. } => name.to_string(),
                        _ => String::new(),
                    })
                    .ok_or_else(|| AegisError::Parse("DELETE missing table".to_string()))?
            }
            sp::FromTable::WithoutKeyword(tables) => {
                tables.first()
                    .map(|t| match &t.relation {
                        sp::TableFactor::Table { name, .. } => name.to_string(),
                        _ => String::new(),
                    })
                    .ok_or_else(|| AegisError::Parse("DELETE missing table".to_string()))?
            }
        };

        let where_clause = delete.selection.map(|e| self.convert_expr(e)).transpose()?;

        Ok(DeleteStatement {
            table,
            where_clause,
        })
    }

    fn convert_create_table(&self, create: sp::CreateTable) -> Result<CreateTableStatement> {
        let columns = create
            .columns
            .into_iter()
            .map(|col| {
                Ok(ColumnDefinition {
                    name: col.name.value,
                    data_type: self.convert_data_type(&col.data_type)?,
                    nullable: !col.options.iter().any(|o| {
                        matches!(o.option, sp::ColumnOption::NotNull)
                    }),
                    default: col
                        .options
                        .iter()
                        .find_map(|o| match &o.option {
                            sp::ColumnOption::Default(e) => Some(self.convert_expr(e.clone())),
                            _ => None,
                        })
                        .transpose()?,
                    constraints: Vec::new(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(CreateTableStatement {
            name: create.name.to_string(),
            columns,
            constraints: Vec::new(),
            if_not_exists: create.if_not_exists,
        })
    }

    fn convert_drop(
        &self,
        object_type: sp::ObjectType,
        if_exists: bool,
        names: Vec<sp::ObjectName>,
    ) -> Result<Statement> {
        let name = names
            .first()
            .map(|n| n.to_string())
            .ok_or_else(|| AegisError::Parse("DROP missing name".to_string()))?;

        match object_type {
            sp::ObjectType::Table => Ok(Statement::DropTable(DropTableStatement {
                name,
                if_exists,
            })),
            sp::ObjectType::Index => Ok(Statement::DropIndex(DropIndexStatement {
                name,
                if_exists,
            })),
            _ => Err(AegisError::Parse(format!(
                "Unsupported DROP object type: {:?}",
                object_type
            ))),
        }
    }

    fn convert_create_index(&self, create: sp::CreateIndex) -> Result<CreateIndexStatement> {
        let name = create
            .name
            .map(|n| n.to_string())
            .ok_or_else(|| AegisError::Parse("CREATE INDEX missing name".to_string()))?;

        let table = create.table_name.to_string();

        let columns = create
            .columns
            .into_iter()
            .map(|c| c.expr.to_string())
            .collect();

        Ok(CreateIndexStatement {
            name,
            table,
            columns,
            unique: create.unique,
            if_not_exists: create.if_not_exists,
        })
    }

    fn convert_alter_table(
        &self,
        name: sp::ObjectName,
        operations: Vec<sp::AlterTableOperation>,
    ) -> Result<AlterTableStatement> {
        let ops = operations
            .into_iter()
            .map(|op| self.convert_alter_operation(op))
            .collect::<Result<Vec<_>>>()?;

        Ok(AlterTableStatement {
            name: name.to_string(),
            operations: ops,
        })
    }

    fn convert_alter_operation(&self, op: sp::AlterTableOperation) -> Result<AlterTableOperation> {
        match op {
            sp::AlterTableOperation::AddColumn { column_def, .. } => {
                let col = ColumnDefinition {
                    name: column_def.name.value.clone(),
                    data_type: self.convert_data_type(&column_def.data_type)?,
                    nullable: !column_def.options.iter().any(|o| {
                        matches!(o.option, sp::ColumnOption::NotNull)
                    }),
                    default: column_def
                        .options
                        .iter()
                        .find_map(|o| match &o.option {
                            sp::ColumnOption::Default(e) => Some(self.convert_expr(e.clone())),
                            _ => None,
                        })
                        .transpose()?,
                    constraints: Vec::new(),
                };
                Ok(AlterTableOperation::AddColumn(col))
            }
            sp::AlterTableOperation::DropColumn { column_name, if_exists, .. } => {
                Ok(AlterTableOperation::DropColumn {
                    name: column_name.value,
                    if_exists,
                })
            }
            sp::AlterTableOperation::RenameColumn { old_column_name, new_column_name } => {
                Ok(AlterTableOperation::RenameColumn {
                    old_name: old_column_name.value,
                    new_name: new_column_name.value,
                })
            }
            sp::AlterTableOperation::RenameTable { table_name } => {
                Ok(AlterTableOperation::RenameTable {
                    new_name: table_name.to_string(),
                })
            }
            sp::AlterTableOperation::AlterColumn { column_name, op } => {
                match op {
                    sp::AlterColumnOperation::SetDataType { data_type, .. } => {
                        Ok(AlterTableOperation::AlterColumn {
                            name: column_name.value,
                            data_type: Some(self.convert_data_type(&data_type)?),
                            set_not_null: None,
                            set_default: None,
                        })
                    }
                    sp::AlterColumnOperation::SetNotNull => {
                        Ok(AlterTableOperation::AlterColumn {
                            name: column_name.value,
                            data_type: None,
                            set_not_null: Some(true),
                            set_default: None,
                        })
                    }
                    sp::AlterColumnOperation::DropNotNull => {
                        Ok(AlterTableOperation::AlterColumn {
                            name: column_name.value,
                            data_type: None,
                            set_not_null: Some(false),
                            set_default: None,
                        })
                    }
                    sp::AlterColumnOperation::SetDefault { value } => {
                        Ok(AlterTableOperation::AlterColumn {
                            name: column_name.value,
                            data_type: None,
                            set_not_null: None,
                            set_default: Some(Some(self.convert_expr(value)?)),
                        })
                    }
                    sp::AlterColumnOperation::DropDefault => {
                        Ok(AlterTableOperation::AlterColumn {
                            name: column_name.value,
                            data_type: None,
                            set_not_null: None,
                            set_default: Some(None),
                        })
                    }
                    _ => Err(AegisError::Parse(format!(
                        "Unsupported ALTER COLUMN operation: {:?}",
                        op
                    ))),
                }
            }
            sp::AlterTableOperation::DropConstraint { name, .. } => {
                Ok(AlterTableOperation::DropConstraint {
                    name: name.value,
                })
            }
            _ => Err(AegisError::Parse(format!(
                "Unsupported ALTER TABLE operation: {:?}",
                op
            ))),
        }
    }

    fn convert_data_type(&self, dt: &sp::DataType) -> Result<DataType> {
        match dt {
            sp::DataType::Boolean => Ok(DataType::Boolean),
            sp::DataType::TinyInt(_) => Ok(DataType::TinyInt),
            sp::DataType::SmallInt(_) => Ok(DataType::SmallInt),
            sp::DataType::Int(_) | sp::DataType::Integer(_) => Ok(DataType::Integer),
            sp::DataType::BigInt(_) => Ok(DataType::BigInt),
            sp::DataType::Real => Ok(DataType::Float),
            sp::DataType::Float(_) | sp::DataType::Double | sp::DataType::DoublePrecision => {
                Ok(DataType::Double)
            }
            sp::DataType::Decimal(info) | sp::DataType::Numeric(info) => {
                let (precision, scale) = match info {
                    sp::ExactNumberInfo::PrecisionAndScale(p, s) => (*p as u8, *s as u8),
                    sp::ExactNumberInfo::Precision(p) => (*p as u8, 0),
                    sp::ExactNumberInfo::None => (18, 0),
                };
                Ok(DataType::Decimal { precision, scale })
            }
            sp::DataType::Char(len) => {
                let size = len.as_ref().and_then(|l| {
                    match l {
                        sp::CharacterLength::IntegerLength { length, .. } => Some(*length as u16),
                        sp::CharacterLength::Max => None,
                    }
                }).unwrap_or(1);
                Ok(DataType::Char(size))
            }
            sp::DataType::Varchar(len) => {
                let size = len.as_ref().and_then(|l| {
                    match l {
                        sp::CharacterLength::IntegerLength { length, .. } => Some(*length as u16),
                        sp::CharacterLength::Max => None,
                    }
                }).unwrap_or(255);
                Ok(DataType::Varchar(size))
            }
            sp::DataType::Text => Ok(DataType::Text),
            sp::DataType::Blob(_) => Ok(DataType::Blob),
            sp::DataType::Date => Ok(DataType::Date),
            sp::DataType::Time(..) => Ok(DataType::Time),
            sp::DataType::Timestamp(..) => Ok(DataType::Timestamp),
            sp::DataType::JSON => Ok(DataType::Json),
            sp::DataType::Uuid => Ok(DataType::Uuid),
            _ => Err(AegisError::Parse(format!("Unsupported data type: {:?}", dt))),
        }
    }

    fn extract_limit(&self, expr: sp::Expr) -> Result<u64> {
        match expr {
            sp::Expr::Value(sp::Value::Number(n, _)) => n
                .parse()
                .map_err(|_| AegisError::Parse("Invalid LIMIT value".to_string())),
            _ => Err(AegisError::Parse("LIMIT must be a number".to_string())),
        }
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let parser = Parser::new();
        let stmt = parser.parse_single("SELECT id, name FROM users").unwrap();

        match stmt {
            Statement::Select(select) => {
                assert_eq!(select.columns.len(), 2);
                assert!(select.from.is_some());
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_select_with_where() {
        let parser = Parser::new();
        let stmt = parser
            .parse_single("SELECT * FROM users WHERE age > 18")
            .unwrap();

        match stmt {
            Statement::Select(select) => {
                assert!(select.where_clause.is_some());
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_select_with_join() {
        let parser = Parser::new();
        let stmt = parser
            .parse_single("SELECT u.name, o.total FROM users u JOIN orders o ON u.id = o.user_id")
            .unwrap();

        match stmt {
            Statement::Select(select) => {
                let from = select.from.unwrap();
                assert_eq!(from.joins.len(), 1);
                assert_eq!(from.joins[0].join_type, JoinType::Inner);
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_insert() {
        let parser = Parser::new();
        let stmt = parser
            .parse_single("INSERT INTO users (name, age) VALUES ('Alice', 25)")
            .unwrap();

        match stmt {
            Statement::Insert(insert) => {
                assert_eq!(insert.table, "users");
                assert_eq!(insert.columns, Some(vec!["name".to_string(), "age".to_string()]));
            }
            _ => panic!("Expected INSERT statement"),
        }
    }

    #[test]
    fn test_parse_update() {
        let parser = Parser::new();
        let stmt = parser
            .parse_single("UPDATE users SET age = 26 WHERE name = 'Alice'")
            .unwrap();

        match stmt {
            Statement::Update(update) => {
                assert_eq!(update.table, "users");
                assert_eq!(update.assignments.len(), 1);
                assert!(update.where_clause.is_some());
            }
            _ => panic!("Expected UPDATE statement"),
        }
    }

    #[test]
    fn test_parse_delete() {
        let parser = Parser::new();
        let stmt = parser
            .parse_single("DELETE FROM users WHERE age < 18")
            .unwrap();

        match stmt {
            Statement::Delete(delete) => {
                assert_eq!(delete.table, "users");
                assert!(delete.where_clause.is_some());
            }
            _ => panic!("Expected DELETE statement"),
        }
    }

    #[test]
    fn test_parse_create_table() {
        let parser = Parser::new();
        let stmt = parser
            .parse_single(
                "CREATE TABLE users (
                    id INTEGER NOT NULL,
                    name VARCHAR(255),
                    age INTEGER
                )",
            )
            .unwrap();

        match stmt {
            Statement::CreateTable(create) => {
                assert_eq!(create.name, "users");
                assert_eq!(create.columns.len(), 3);
                assert!(!create.columns[0].nullable);
                assert!(create.columns[1].nullable);
            }
            _ => panic!("Expected CREATE TABLE statement"),
        }
    }

    #[test]
    fn test_parse_transaction_statements() {
        let parser = Parser::new();

        assert!(matches!(parser.parse_single("BEGIN").unwrap(), Statement::Begin));
        assert!(matches!(parser.parse_single("COMMIT").unwrap(), Statement::Commit));
        assert!(matches!(parser.parse_single("ROLLBACK").unwrap(), Statement::Rollback));
    }
}

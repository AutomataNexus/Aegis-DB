//! Aegis Analyzer - Semantic Analysis
//!
//! Performs semantic analysis on parsed SQL statements. Validates table and
//! column references, resolves types, and checks for constraint violations.
//!
//! Key Features:
//! - Table and column resolution
//! - Type checking and inference
//! - Constraint validation
//! - Scope management for subqueries
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::ast::*;
use aegis_common::{AegisError, DataType, Result};
use std::collections::HashMap;

// =============================================================================
// Catalog
// =============================================================================

/// Schema information for analysis.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    tables: HashMap<String, TableSchema>,
}

impl Catalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_table(&mut self, schema: TableSchema) {
        self.tables.insert(schema.name.clone(), schema);
    }

    pub fn get_table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.get(name)
    }

    pub fn table_exists(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }
}

/// Schema for a table.
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
}

impl TableSchema {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            columns: Vec::new(),
        }
    }

    pub fn add_column(&mut self, column: ColumnSchema) {
        self.columns.push(column);
    }

    pub fn get_column(&self, name: &str) -> Option<&ColumnSchema> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn column_exists(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }
}

/// Schema for a column.
#[derive(Debug, Clone)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

// =============================================================================
// Analysis Context
// =============================================================================

/// Context for semantic analysis.
#[derive(Debug)]
#[allow(dead_code)]
struct AnalysisContext<'a> {
    catalog: &'a Catalog,
    scope: Scope,
}

/// Scope for name resolution.
#[derive(Debug, Default)]
struct Scope {
    tables: HashMap<String, String>,
    columns: HashMap<String, ResolvedColumn>,
}

/// A resolved column reference.
#[derive(Debug, Clone)]
struct ResolvedColumn {
    table: String,
    column: String,
    data_type: DataType,
}

// =============================================================================
// Analyzer
// =============================================================================

/// Semantic analyzer for SQL statements.
pub struct Analyzer {
    catalog: Catalog,
}

impl Analyzer {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    /// Analyze a statement for semantic correctness.
    pub fn analyze(&self, stmt: &Statement) -> Result<AnalyzedStatement> {
        match stmt {
            Statement::Select(select) => {
                let analyzed = self.analyze_select(select)?;
                Ok(AnalyzedStatement::Select(analyzed))
            }
            Statement::Insert(insert) => {
                self.analyze_insert(insert)?;
                Ok(AnalyzedStatement::Insert(insert.clone()))
            }
            Statement::Update(update) => {
                self.analyze_update(update)?;
                Ok(AnalyzedStatement::Update(update.clone()))
            }
            Statement::Delete(delete) => {
                self.analyze_delete(delete)?;
                Ok(AnalyzedStatement::Delete(delete.clone()))
            }
            Statement::CreateTable(create) => {
                self.analyze_create_table(create)?;
                Ok(AnalyzedStatement::CreateTable(create.clone()))
            }
            Statement::DropTable(drop) => {
                Ok(AnalyzedStatement::DropTable(drop.clone()))
            }
            Statement::CreateIndex(create) => {
                self.analyze_create_index(create)?;
                Ok(AnalyzedStatement::CreateIndex(create.clone()))
            }
            Statement::DropIndex(drop) => {
                Ok(AnalyzedStatement::DropIndex(drop.clone()))
            }
            Statement::Begin => Ok(AnalyzedStatement::Begin),
            Statement::Commit => Ok(AnalyzedStatement::Commit),
            Statement::Rollback => Ok(AnalyzedStatement::Rollback),
        }
    }

    fn analyze_select(&self, select: &SelectStatement) -> Result<AnalyzedSelect> {
        let mut scope = Scope::default();

        if let Some(ref from) = select.from {
            self.build_scope_from_clause(from, &mut scope)?;
        }

        let mut output_columns = Vec::new();
        for col in &select.columns {
            match col {
                SelectColumn::AllColumns => {
                    for (_, resolved) in &scope.columns {
                        output_columns.push(OutputColumn {
                            name: resolved.column.clone(),
                            data_type: resolved.data_type.clone(),
                        });
                    }
                }
                SelectColumn::TableAllColumns(table) => {
                    for (_, resolved) in &scope.columns {
                        if resolved.table == *table {
                            output_columns.push(OutputColumn {
                                name: resolved.column.clone(),
                                data_type: resolved.data_type.clone(),
                            });
                        }
                    }
                }
                SelectColumn::Expression { expr, alias } => {
                    let data_type = self.infer_type(expr, &scope)?;
                    let name = alias.clone().unwrap_or_else(|| self.expr_name(expr));
                    output_columns.push(OutputColumn { name, data_type });
                }
            }
        }

        if let Some(ref where_clause) = select.where_clause {
            self.validate_expression(where_clause, &scope)?;
        }

        for expr in &select.group_by {
            self.validate_expression(expr, &scope)?;
        }

        if let Some(ref having) = select.having {
            self.validate_expression(having, &scope)?;
        }

        for order_by in &select.order_by {
            self.validate_expression(&order_by.expression, &scope)?;
        }

        Ok(AnalyzedSelect {
            statement: select.clone(),
            output_columns,
        })
    }

    fn analyze_insert(&self, insert: &InsertStatement) -> Result<()> {
        let table = self.catalog.get_table(&insert.table).ok_or_else(|| {
            AegisError::TableNotFound(insert.table.clone())
        })?;

        if let Some(ref columns) = insert.columns {
            for col_name in columns {
                if !table.column_exists(col_name) {
                    return Err(AegisError::ColumnNotFound(col_name.clone()));
                }
            }
        }

        Ok(())
    }

    fn analyze_update(&self, update: &UpdateStatement) -> Result<()> {
        let table = self.catalog.get_table(&update.table).ok_or_else(|| {
            AegisError::TableNotFound(update.table.clone())
        })?;

        for assignment in &update.assignments {
            if !table.column_exists(&assignment.column) {
                return Err(AegisError::ColumnNotFound(assignment.column.clone()));
            }
        }

        Ok(())
    }

    fn analyze_delete(&self, delete: &DeleteStatement) -> Result<()> {
        if !self.catalog.table_exists(&delete.table) {
            return Err(AegisError::TableNotFound(delete.table.clone()));
        }
        Ok(())
    }

    fn analyze_create_table(&self, create: &CreateTableStatement) -> Result<()> {
        if self.catalog.table_exists(&create.name) && !create.if_not_exists {
            return Err(AegisError::ConstraintViolation(format!(
                "Table '{}' already exists",
                create.name
            )));
        }
        Ok(())
    }

    fn analyze_create_index(&self, create: &CreateIndexStatement) -> Result<()> {
        let table = self.catalog.get_table(&create.table).ok_or_else(|| {
            AegisError::TableNotFound(create.table.clone())
        })?;

        for col_name in &create.columns {
            if !table.column_exists(col_name) {
                return Err(AegisError::ColumnNotFound(col_name.clone()));
            }
        }

        Ok(())
    }

    fn build_scope_from_clause(&self, from: &FromClause, scope: &mut Scope) -> Result<()> {
        self.add_table_to_scope(&from.source, scope)?;

        for join in &from.joins {
            self.add_table_to_scope(&join.table, scope)?;
        }

        Ok(())
    }

    fn add_table_to_scope(&self, table_ref: &TableReference, scope: &mut Scope) -> Result<()> {
        match table_ref {
            TableReference::Table { name, alias } => {
                let table = self.catalog.get_table(name).ok_or_else(|| {
                    AegisError::TableNotFound(name.clone())
                })?;

                let alias_name = alias.as_ref().unwrap_or(name);
                scope.tables.insert(alias_name.clone(), name.clone());

                for col in &table.columns {
                    let key = format!("{}.{}", alias_name, col.name);
                    scope.columns.insert(
                        key.clone(),
                        ResolvedColumn {
                            table: alias_name.clone(),
                            column: col.name.clone(),
                            data_type: col.data_type.clone(),
                        },
                    );

                    if !scope.columns.contains_key(&col.name) {
                        scope.columns.insert(
                            col.name.clone(),
                            ResolvedColumn {
                                table: alias_name.clone(),
                                column: col.name.clone(),
                                data_type: col.data_type.clone(),
                            },
                        );
                    }
                }
            }
            TableReference::Subquery { query: _, alias } => {
                scope.tables.insert(alias.clone(), alias.clone());
            }
        }

        Ok(())
    }

    fn validate_expression(&self, expr: &Expression, scope: &Scope) -> Result<DataType> {
        self.infer_type(expr, scope)
    }

    fn infer_type(&self, expr: &Expression, scope: &Scope) -> Result<DataType> {
        match expr {
            Expression::Literal(lit) => Ok(self.literal_type(lit)),
            Expression::Column(col_ref) => {
                let key = if let Some(ref table) = col_ref.table {
                    format!("{}.{}", table, col_ref.column)
                } else {
                    col_ref.column.clone()
                };

                scope
                    .columns
                    .get(&key)
                    .map(|r| r.data_type.clone())
                    .ok_or_else(|| AegisError::ColumnNotFound(key))
            }
            Expression::BinaryOp { left, op, right } => {
                let left_type = self.infer_type(left, scope)?;
                let right_type = self.infer_type(right, scope)?;
                self.binary_op_type(&left_type, op, &right_type)
            }
            Expression::UnaryOp { op, expr } => {
                let expr_type = self.infer_type(expr, scope)?;
                self.unary_op_type(op, &expr_type)
            }
            Expression::Function { name, args, .. } => {
                for arg in args {
                    self.infer_type(arg, scope)?;
                }
                self.function_return_type(name, args, scope)
            }
            Expression::IsNull { .. } => Ok(DataType::Boolean),
            Expression::InList { .. } => Ok(DataType::Boolean),
            Expression::Between { .. } => Ok(DataType::Boolean),
            Expression::Like { .. } => Ok(DataType::Boolean),
            Expression::Cast { data_type, .. } => Ok(data_type.clone()),
            Expression::Case { conditions, else_result, .. } => {
                if let Some((_, then_expr)) = conditions.first() {
                    self.infer_type(then_expr, scope)
                } else if let Some(else_expr) = else_result {
                    self.infer_type(else_expr, scope)
                } else {
                    Ok(DataType::Text)
                }
            }
            Expression::Subquery(_) => Ok(DataType::Text),
            Expression::InSubquery { .. } => Ok(DataType::Boolean),
            Expression::Exists { .. } => Ok(DataType::Boolean),
            Expression::Placeholder(_) => Ok(DataType::Text),
        }
    }

    fn literal_type(&self, lit: &Literal) -> DataType {
        match lit {
            Literal::Null => DataType::Text,
            Literal::Boolean(_) => DataType::Boolean,
            Literal::Integer(_) => DataType::BigInt,
            Literal::Float(_) => DataType::Double,
            Literal::String(_) => DataType::Text,
        }
    }

    fn binary_op_type(
        &self,
        left: &DataType,
        op: &BinaryOperator,
        _right: &DataType,
    ) -> Result<DataType> {
        match op {
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::LessThan
            | BinaryOperator::LessThanOrEqual
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterThanOrEqual
            | BinaryOperator::And
            | BinaryOperator::Or => Ok(DataType::Boolean),
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo => Ok(left.clone()),
            BinaryOperator::Concat => Ok(DataType::Text),
        }
    }

    fn unary_op_type(&self, op: &UnaryOperator, expr_type: &DataType) -> Result<DataType> {
        match op {
            UnaryOperator::Not => Ok(DataType::Boolean),
            UnaryOperator::Negative | UnaryOperator::Positive => Ok(expr_type.clone()),
        }
    }

    fn function_return_type(
        &self,
        name: &str,
        _args: &[Expression],
        _scope: &Scope,
    ) -> Result<DataType> {
        let name_upper = name.to_uppercase();
        match name_upper.as_str() {
            "COUNT" => Ok(DataType::BigInt),
            "SUM" | "AVG" => Ok(DataType::Double),
            "MIN" | "MAX" => Ok(DataType::Double),
            "COALESCE" | "NULLIF" => Ok(DataType::Text),
            "NOW" | "CURRENT_TIMESTAMP" => Ok(DataType::Timestamp),
            "CURRENT_DATE" => Ok(DataType::Date),
            "UPPER" | "LOWER" | "TRIM" | "CONCAT" | "SUBSTRING" => Ok(DataType::Text),
            "LENGTH" | "CHAR_LENGTH" => Ok(DataType::Integer),
            "ABS" | "CEIL" | "FLOOR" | "ROUND" => Ok(DataType::Double),
            _ => Ok(DataType::Text),
        }
    }

    fn expr_name(&self, expr: &Expression) -> String {
        match expr {
            Expression::Column(col) => col.column.clone(),
            Expression::Function { name, .. } => name.clone(),
            Expression::Literal(lit) => match lit {
                Literal::Integer(i) => i.to_string(),
                Literal::Float(f) => f.to_string(),
                Literal::String(s) => s.clone(),
                Literal::Boolean(b) => b.to_string(),
                Literal::Null => "NULL".to_string(),
            },
            _ => "?column?".to_string(),
        }
    }
}

// =============================================================================
// Analyzed Output
// =============================================================================

/// Result of semantic analysis.
#[derive(Debug, Clone)]
pub enum AnalyzedStatement {
    Select(AnalyzedSelect),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    CreateTable(CreateTableStatement),
    DropTable(DropTableStatement),
    CreateIndex(CreateIndexStatement),
    DropIndex(DropIndexStatement),
    Begin,
    Commit,
    Rollback,
}

/// Analyzed SELECT statement with resolved types.
#[derive(Debug, Clone)]
pub struct AnalyzedSelect {
    pub statement: SelectStatement,
    pub output_columns: Vec<OutputColumn>,
}

/// Output column with resolved type.
#[derive(Debug, Clone)]
pub struct OutputColumn {
    pub name: String,
    pub data_type: DataType,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn create_test_catalog() -> Catalog {
        let mut catalog = Catalog::new();

        let mut users = TableSchema::new("users");
        users.add_column(ColumnSchema {
            name: "id".to_string(),
            data_type: DataType::Integer,
            nullable: false,
        });
        users.add_column(ColumnSchema {
            name: "name".to_string(),
            data_type: DataType::Varchar(255),
            nullable: true,
        });
        users.add_column(ColumnSchema {
            name: "age".to_string(),
            data_type: DataType::Integer,
            nullable: true,
        });
        catalog.add_table(users);

        catalog
    }

    #[test]
    fn test_analyze_select() {
        let catalog = create_test_catalog();
        let analyzer = Analyzer::new(catalog);
        let parser = Parser::new();

        let stmt = parser.parse_single("SELECT id, name FROM users").unwrap();
        let analyzed = analyzer.analyze(&stmt).unwrap();

        match analyzed {
            AnalyzedStatement::Select(select) => {
                assert_eq!(select.output_columns.len(), 2);
                assert_eq!(select.output_columns[0].name, "id");
                assert_eq!(select.output_columns[0].data_type, DataType::Integer);
            }
            _ => panic!("Expected analyzed SELECT"),
        }
    }

    #[test]
    fn test_analyze_table_not_found() {
        let catalog = create_test_catalog();
        let analyzer = Analyzer::new(catalog);
        let parser = Parser::new();

        let stmt = parser.parse_single("SELECT * FROM nonexistent").unwrap();
        let result = analyzer.analyze(&stmt);

        assert!(matches!(result, Err(AegisError::TableNotFound(_))));
    }

    #[test]
    fn test_analyze_column_not_found() {
        let catalog = create_test_catalog();
        let analyzer = Analyzer::new(catalog);
        let parser = Parser::new();

        let stmt = parser.parse_single("SELECT nonexistent FROM users").unwrap();
        let result = analyzer.analyze(&stmt);

        assert!(matches!(result, Err(AegisError::ColumnNotFound(_))));
    }
}

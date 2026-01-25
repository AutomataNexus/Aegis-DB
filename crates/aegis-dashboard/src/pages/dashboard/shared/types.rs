//! Shared types for the dashboard

/// Dashboard page type
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum DashboardPage {
    #[default]
    Overview,
    Nodes,
    Database,
    Metrics,
    Settings,
}

/// Modal type for database browsers
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum BrowserModal {
    #[default]
    None,
    KeyValue,
    Collections,
    Graph,
    QueryBuilder,
    NodeDetail,
    DataVisualizer,
}

/// Chart types for data visualization
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum DataChartType {
    #[default]
    Scatter3D,
    ScatterPlot,
    LineChart,
    BarChart,
    AreaChart,
    RadarChart,
    HeatMap,
    BubbleChart,
}

/// Token types for syntax highlighting
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TokenType {
    Keyword,
    String,
    Number,
    Operator,
    Identifier,
    Punctuation,
    Comment,
    Function,
    Type,
    Whitespace,
}

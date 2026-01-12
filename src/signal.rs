/// Type-safe representation of telemetry signal types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Signal {
    Logs,
    Traces,
    Gauge,
    Sum,
    Histogram,
    ExpHistogram,
    Summary,
}

impl Signal {
    /// Environment variable name for pipeline endpoint
    pub fn env_var_name(&self) -> &'static str {
        match self {
            Signal::Logs => "PIPELINE_LOGS",
            Signal::Traces => "PIPELINE_TRACES",
            Signal::Gauge => "PIPELINE_GAUGE",
            Signal::Sum => "PIPELINE_SUM",
            Signal::Histogram => "PIPELINE_HISTOGRAM",
            Signal::ExpHistogram => "PIPELINE_EXP_HISTOGRAM",
            Signal::Summary => "PIPELINE_SUMMARY",
        }
    }

    /// Table name used in routing (matches VRL's `._table` assignment)
    pub fn table_name(&self) -> &'static str {
        match self {
            Signal::Logs => "logs",
            Signal::Traces => "traces",
            Signal::Gauge => "gauge",
            Signal::Sum => "sum",
            Signal::Histogram => "histogram",
            Signal::ExpHistogram => "exp_histogram",
            Signal::Summary => "summary",
        }
    }

    /// All supported signal types
    pub fn all() -> &'static [Signal] {
        &[
            Signal::Logs,
            Signal::Traces,
            Signal::Gauge,
            Signal::Sum,
            Signal::Histogram,
            Signal::ExpHistogram,
            Signal::Summary,
        ]
    }

    /// Parse from table name string
    pub fn from_table_name(name: &str) -> Option<Signal> {
        match name {
            "logs" => Some(Signal::Logs),
            "traces" => Some(Signal::Traces),
            "gauge" => Some(Signal::Gauge),
            "sum" => Some(Signal::Sum),
            "histogram" => Some(Signal::Histogram),
            "exp_histogram" => Some(Signal::ExpHistogram),
            "summary" => Some(Signal::Summary),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_round_trips_through_table_name() {
        for signal in Signal::all() {
            let name = signal.table_name();
            let parsed = Signal::from_table_name(name);
            assert_eq!(parsed, Some(*signal));
        }
    }

    #[test]
    fn env_var_names_are_uppercase() {
        for signal in Signal::all() {
            let env_var = signal.env_var_name();
            assert!(env_var.starts_with("PIPELINE_"));
            assert_eq!(env_var, env_var.to_uppercase());
        }
    }
}

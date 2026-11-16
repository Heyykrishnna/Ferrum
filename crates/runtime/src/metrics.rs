use std::{
    collections::BTreeMap,
    fmt::Write,
    sync::{
        OnceLock, RwLock,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
};

pub static SECONDS_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

pub static MILLIS_BUCKETS: &[f64] = &[
    1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
];

pub static MEMORY_KIB_BUCKETS: &[f64] = &[
    1024.0, 4096.0, 16384.0, 32768.0, 65536.0, 131072.0, 262144.0, 524288.0,
];

pub struct AtomicCounter(AtomicU64);

impl Default for AtomicCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicCounter {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn inc(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_by(&self, val: u64) {
        self.0.fetch_add(val, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

pub struct AtomicGauge(AtomicI64);

impl Default for AtomicGauge {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicGauge {
    pub const fn new() -> Self {
        Self(AtomicI64::new(0))
    }

    pub fn set(&self, val: i64) {
        self.0.store(val, Ordering::Relaxed);
    }

    pub fn inc(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec(&self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn add(&self, val: i64) {
        self.0.fetch_add(val, Ordering::Relaxed);
    }

    pub fn sub(&self, val: i64) {
        self.0.fetch_sub(val, Ordering::Relaxed);
    }

    pub fn get(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }
}

pub struct AtomicHistogram {
    buckets: &'static [f64],
    counts: Vec<AtomicU64>,
    sum_micros: AtomicU64,
    total_count: AtomicU64,
}

impl AtomicHistogram {
    pub fn new(buckets: &'static [f64]) -> Self {
        let mut counts = Vec::with_capacity(buckets.len());
        for _ in 0..buckets.len() {
            counts.push(AtomicU64::new(0));
        }
        Self {
            buckets,
            counts,
            sum_micros: AtomicU64::new(0),
            total_count: AtomicU64::new(0),
        }
    }

    pub fn observe(&self, value: f64) {
        for (i, &bound) in self.buckets.iter().enumerate() {
            if value <= bound {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        let micros = (value * 1_000_000.0).max(0.0) as u64;
        self.sum_micros.fetch_add(micros, Ordering::Relaxed);
        self.total_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> (Vec<(&'static str, u64)>, f64, u64) {
        let mut bucket_counts = Vec::with_capacity(self.buckets.len());
        for (i, &bound) in self.buckets.iter().enumerate() {
            let label = if bound.fract() == 0.0 {
                Box::leak(format!("{bound:.1}").into_boxed_str())
            } else {
                Box::leak(format!("{bound}").into_boxed_str())
            };
            bucket_counts.push((label as &'static str, self.counts[i].load(Ordering::Relaxed)));
        }
        let sum = self.sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let count = self.total_count.load(Ordering::Relaxed);
        (bucket_counts, sum, count)
    }
}

type LabelMap = BTreeMap<&'static str, String>;

pub struct CounterFamily {
    name: &'static str,
    help: &'static str,
    metrics: RwLock<BTreeMap<LabelMap, AtomicCounter>>,
}

impl CounterFamily {
    pub const fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            metrics: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn with_labels(&self, labels: &[(&'static str, &str)]) -> u64 {
        let key = Self::labels_to_map(labels);
        {
            let read = self.metrics.read().unwrap();
            if let Some(counter) = read.get(&key) {
                return counter.get();
            }
        }
        let mut write = self.metrics.write().unwrap();
        write.entry(key).or_default().get()
    }

    pub fn inc(&self, labels: &[(&'static str, &str)]) {
        self.inc_by(labels, 1);
    }

    pub fn inc_by(&self, labels: &[(&'static str, &str)], val: u64) {
        let key = Self::labels_to_map(labels);
        {
            let read = self.metrics.read().unwrap();
            if let Some(counter) = read.get(&key) {
                counter.inc_by(val);
                return;
            }
        }
        let mut write = self.metrics.write().unwrap();
        let counter = write.entry(key).or_default();
        counter.inc_by(val);
    }

    fn labels_to_map(labels: &[(&'static str, &str)]) -> LabelMap {
        let mut map = BTreeMap::new();
        for &(k, v) in labels {
            map.insert(k, v.to_string());
        }
        map
    }

    pub fn render(&self, out: &mut String) {
        let read = self.metrics.read().unwrap();
        if read.is_empty() {
            return;
        }
        let _ = writeln!(out, "# HELP {} {}", self.name, self.help);
        let _ = writeln!(out, "# TYPE {} counter", self.name);
        for (labels, counter) in read.iter() {
            out.push_str(self.name);
            render_labels(labels, out);
            let _ = writeln!(out, " {}", counter.get());
        }
    }
}

pub struct GaugeFamily {
    name: &'static str,
    help: &'static str,
    metrics: RwLock<BTreeMap<LabelMap, AtomicGauge>>,
}

impl GaugeFamily {
    pub const fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            metrics: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn set(&self, labels: &[(&'static str, &str)], val: i64) {
        let key = CounterFamily::labels_to_map(labels);
        {
            let read = self.metrics.read().unwrap();
            if let Some(gauge) = read.get(&key) {
                gauge.set(val);
                return;
            }
        }
        let mut write = self.metrics.write().unwrap();
        let gauge = write.entry(key).or_default();
        gauge.set(val);
    }

    pub fn inc(&self, labels: &[(&'static str, &str)]) {
        self.add(labels, 1);
    }

    pub fn dec(&self, labels: &[(&'static str, &str)]) {
        self.add(labels, -1);
    }

    pub fn add(&self, labels: &[(&'static str, &str)], val: i64) {
        let key = CounterFamily::labels_to_map(labels);
        {
            let read = self.metrics.read().unwrap();
            if let Some(gauge) = read.get(&key) {
                gauge.add(val);
                return;
            }
        }
        let mut write = self.metrics.write().unwrap();
        let gauge = write.entry(key).or_default();
        gauge.add(val);
    }

    pub fn render(&self, out: &mut String) {
        let read = self.metrics.read().unwrap();
        if read.is_empty() {
            return;
        }
        let _ = writeln!(out, "# HELP {} {}", self.name, self.help);
        let _ = writeln!(out, "# TYPE {} gauge", self.name);
        for (labels, gauge) in read.iter() {
            out.push_str(self.name);
            render_labels(labels, out);
            let _ = writeln!(out, " {}", gauge.get());
        }
    }
}

pub struct HistogramFamily {
    name: &'static str,
    help: &'static str,
    buckets: &'static [f64],
    metrics: RwLock<BTreeMap<LabelMap, AtomicHistogram>>,
}

impl HistogramFamily {
    pub const fn new(name: &'static str, help: &'static str, buckets: &'static [f64]) -> Self {
        Self {
            name,
            help,
            buckets,
            metrics: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn observe(&self, labels: &[(&'static str, &str)], val: f64) {
        let key = CounterFamily::labels_to_map(labels);
        {
            let read = self.metrics.read().unwrap();
            if let Some(hist) = read.get(&key) {
                hist.observe(val);
                return;
            }
        }
        let mut write = self.metrics.write().unwrap();
        let hist = write
            .entry(key)
            .or_insert_with(|| AtomicHistogram::new(self.buckets));
        hist.observe(val);
    }

    pub fn render(&self, out: &mut String) {
        let read = self.metrics.read().unwrap();
        if read.is_empty() {
            return;
        }
        let _ = writeln!(out, "# HELP {} {}", self.name, self.help);
        let _ = writeln!(out, "# TYPE {} histogram", self.name);
        for (labels, hist) in read.iter() {
            let (buckets, sum, count) = hist.snapshot();
            for (le, b_count) in buckets {
                let mut b_labels = labels.clone();
                b_labels.insert("le", le.to_string());
                out.push_str(self.name);
                out.push_str("_bucket");
                render_labels(&b_labels, out);
                let _ = writeln!(out, " {b_count}");
            }
            let mut inf_labels = labels.clone();
            inf_labels.insert("le", "+Inf".to_string());
            out.push_str(self.name);
            out.push_str("_bucket");
            render_labels(&inf_labels, out);
            let _ = writeln!(out, " {count}");

            out.push_str(self.name);
            out.push_str("_sum");
            render_labels(labels, out);
            let _ = writeln!(out, " {sum:.6}");

            out.push_str(self.name);
            out.push_str("_count");
            render_labels(labels, out);
            let _ = writeln!(out, " {count}");
        }
    }
}

fn render_labels(labels: &LabelMap, out: &mut String) {
    if labels.is_empty() {
        return;
    }
    out.push('{');
    let mut first = true;
    for (k, v) in labels {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(k);
        out.push_str("=\"");
        for c in v.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                _ => out.push(c),
            }
        }
        out.push('"');
    }
    out.push('}');
}

pub struct MetricsRegistry {
    pub http_requests_total: CounterFamily,
    pub http_request_duration_seconds: HistogramFamily,
    pub auth_failures_total: CounterFamily,
    pub rate_limit_rejections_total: CounterFamily,
    pub submissions_created_total: CounterFamily,

    pub outbox_events_total: CounterFamily,
    pub outbox_dispatch_duration_seconds: HistogramFamily,
    pub outbox_oldest_event_age_seconds: GaugeFamily,
    pub outbox_unpublished_count: GaugeFamily,
    pub reconciliation_runs_total: CounterFamily,
    pub reconciled_items_total: CounterFamily,

    pub worker_active_executions: GaugeFamily,
    pub worker_capacity_reserved_slots: GaugeFamily,
    pub worker_capacity_quarantined_slots: GaugeFamily,
    pub worker_executions_total: CounterFamily,
    pub stage_duration_seconds: HistogramFamily,

    pub sandbox_initialization_duration_seconds: HistogramFamily,
    pub sandbox_cleanup_duration_seconds: HistogramFamily,
    pub sandbox_cleanups_total: CounterFamily,
    pub sandbox_quarantines_total: CounterFamily,
    pub execution_cpu_time_ms: HistogramFamily,
    pub execution_wall_time_ms: HistogramFamily,
    pub execution_peak_memory_kib: HistogramFamily,
    pub execution_limit_violations_total: CounterFamily,

    pub compilation_duration_seconds: HistogramFamily,
    pub compilation_failures_total: CounterFamily,
    pub verdicts_total: CounterFamily,

    pub database_connections_active: GaugeFamily,
    pub database_connections_idle: GaugeFamily,
    pub database_query_duration_seconds: HistogramFamily,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            http_requests_total: CounterFamily::new(
                "ferrum_http_requests_total",
                "Total number of HTTP requests processed by endpoint and status",
            ),
            http_request_duration_seconds: HistogramFamily::new(
                "ferrum_http_request_duration_seconds",
                "HTTP request duration in seconds",
                SECONDS_BUCKETS,
            ),
            auth_failures_total: CounterFamily::new(
                "ferrum_api_auth_failures_total",
                "Total number of API authentication failures",
            ),
            rate_limit_rejections_total: CounterFamily::new(
                "ferrum_api_rate_limit_rejections_total",
                "Total number of rejected requests due to rate limits or quotas",
            ),
            submissions_created_total: CounterFamily::new(
                "ferrum_submissions_created_total",
                "Total submissions received by language",
            ),

            outbox_events_total: CounterFamily::new(
                "ferrum_outbox_events_total",
                "Total outbox events dispatched by type and status",
            ),
            outbox_dispatch_duration_seconds: HistogramFamily::new(
                "ferrum_outbox_dispatch_duration_seconds",
                "Duration of transactional outbox dispatch cycles",
                SECONDS_BUCKETS,
            ),
            outbox_oldest_event_age_seconds: GaugeFamily::new(
                "ferrum_outbox_oldest_event_age_seconds",
                "Age of oldest unpublished outbox event in seconds",
            ),
            outbox_unpublished_count: GaugeFamily::new(
                "ferrum_outbox_unpublished_count",
                "Count of pending unpublished outbox events",
            ),
            reconciliation_runs_total: CounterFamily::new(
                "ferrum_reconciliation_runs_total",
                "Total background reconciliation cycles executed",
            ),
            reconciled_items_total: CounterFamily::new(
                "ferrum_reconciled_items_total",
                "Total orphaned attempts or stuck outbox items reconciled",
            ),

            worker_active_executions: GaugeFamily::new(
                "ferrum_worker_active_executions",
                "Current active running executions on this worker host",
            ),
            worker_capacity_reserved_slots: GaugeFamily::new(
                "ferrum_worker_capacity_reserved_slots",
                "Current execution slots actively reserved",
            ),
            worker_capacity_quarantined_slots: GaugeFamily::new(
                "ferrum_worker_capacity_quarantined_slots",
                "Execution slots currently blocked in quarantine",
            ),
            worker_executions_total: CounterFamily::new(
                "ferrum_worker_executions_total",
                "Total executions processed by worker",
            ),
            stage_duration_seconds: HistogramFamily::new(
                "ferrum_stage_duration_seconds",
                "Execution stage latency decomposition in seconds",
                SECONDS_BUCKETS,
            ),

            sandbox_initialization_duration_seconds: HistogramFamily::new(
                "ferrum_sandbox_initialization_duration_seconds",
                "Time taken to initialize Isolate sandbox",
                SECONDS_BUCKETS,
            ),
            sandbox_cleanup_duration_seconds: HistogramFamily::new(
                "ferrum_sandbox_cleanup_duration_seconds",
                "Time taken to destroy and verify sandbox cleanup",
                SECONDS_BUCKETS,
            ),
            sandbox_cleanups_total: CounterFamily::new(
                "ferrum_sandbox_cleanups_total",
                "Sandbox cleanups by disposition",
            ),
            sandbox_quarantines_total: CounterFamily::new(
                "ferrum_sandbox_quarantines_total",
                "Sandbox cleanups that resulted in quarantine",
            ),
            execution_cpu_time_ms: HistogramFamily::new(
                "ferrum_execution_cpu_time_milliseconds",
                "Program CPU time consumed in milliseconds",
                MILLIS_BUCKETS,
            ),
            execution_wall_time_ms: HistogramFamily::new(
                "ferrum_execution_wall_time_milliseconds",
                "Program wall time consumed in milliseconds",
                MILLIS_BUCKETS,
            ),
            execution_peak_memory_kib: HistogramFamily::new(
                "ferrum_execution_peak_memory_kibibytes",
                "Program peak memory usage in kibibytes",
                MEMORY_KIB_BUCKETS,
            ),
            execution_limit_violations_total: CounterFamily::new(
                "ferrum_execution_limit_violations_total",
                "Total executions terminating due to resource limit violation",
            ),

            compilation_duration_seconds: HistogramFamily::new(
                "ferrum_compilation_duration_seconds",
                "Source compilation duration in seconds",
                SECONDS_BUCKETS,
            ),
            compilation_failures_total: CounterFamily::new(
                "ferrum_compilation_failures_total",
                "Total compilation errors by language",
            ),
            verdicts_total: CounterFamily::new(
                "ferrum_verdicts_total",
                "Total final verdicts computed by language and verdict",
            ),

            database_connections_active: GaugeFamily::new(
                "ferrum_database_connections_active",
                "Active PostgreSQL connections in pool",
            ),
            database_connections_idle: GaugeFamily::new(
                "ferrum_database_connections_idle",
                "Idle PostgreSQL connections in pool",
            ),
            database_query_duration_seconds: HistogramFamily::new(
                "ferrum_database_query_duration_seconds",
                "Database query latency in seconds by operation",
                SECONDS_BUCKETS,
            ),
        }
    }

    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();
        REGISTRY.get_or_init(MetricsRegistry::new)
    }

    pub fn render(&self) -> String {
        let mut out = String::with_capacity(8192);
        self.http_requests_total.render(&mut out);
        self.http_request_duration_seconds.render(&mut out);
        self.auth_failures_total.render(&mut out);
        self.rate_limit_rejections_total.render(&mut out);
        self.submissions_created_total.render(&mut out);

        self.outbox_events_total.render(&mut out);
        self.outbox_dispatch_duration_seconds.render(&mut out);
        self.outbox_oldest_event_age_seconds.render(&mut out);
        self.outbox_unpublished_count.render(&mut out);
        self.reconciliation_runs_total.render(&mut out);
        self.reconciled_items_total.render(&mut out);

        self.worker_active_executions.render(&mut out);
        self.worker_capacity_reserved_slots.render(&mut out);
        self.worker_capacity_quarantined_slots.render(&mut out);
        self.worker_executions_total.render(&mut out);
        self.stage_duration_seconds.render(&mut out);

        self.sandbox_initialization_duration_seconds.render(&mut out);
        self.sandbox_cleanup_duration_seconds.render(&mut out);
        self.sandbox_cleanups_total.render(&mut out);
        self.sandbox_quarantines_total.render(&mut out);
        self.execution_cpu_time_ms.render(&mut out);
        self.execution_wall_time_ms.render(&mut out);
        self.execution_peak_memory_kib.render(&mut out);
        self.execution_limit_violations_total.render(&mut out);

        self.compilation_duration_seconds.render(&mut out);
        self.compilation_failures_total.render(&mut out);
        self.verdicts_total.render(&mut out);

        self.database_connections_active.render(&mut out);
        self.database_connections_idle.render(&mut out);
        self.database_query_duration_seconds.render(&mut out);
        out
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_metrics() -> String {
    MetricsRegistry::global().render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_increments_and_renders() {
        let family = CounterFamily::new("test_counter_total", "Test counter help");
        family.inc(&[("method", "GET"), ("status", "200")]);
        family.inc_by(&[("method", "GET"), ("status", "200")], 4);
        let mut out = String::new();
        family.render(&mut out);
        assert!(out.contains("# HELP test_counter_total Test counter help"));
        assert!(out.contains("# TYPE test_counter_total counter"));
        assert!(out.contains("test_counter_total{method=\"GET\",status=\"200\"} 5"));
    }

    #[test]
    fn gauge_sets_and_renders() {
        let family = GaugeFamily::new("test_gauge", "Test gauge help");
        family.set(&[("pool", "primary")], 10);
        family.inc(&[("pool", "primary")]);
        family.dec(&[("pool", "primary")]);
        let mut out = String::new();
        family.render(&mut out);
        assert!(out.contains("# HELP test_gauge Test gauge help"));
        assert!(out.contains("# TYPE test_gauge gauge"));
        assert!(out.contains("test_gauge{pool=\"primary\"} 10"));
    }

    #[test]
    fn histogram_observes_and_renders() {
        let family = HistogramFamily::new("test_hist_seconds", "Test hist help", &[0.05, 0.1, 0.5]);
        family.observe(&[("stage", "compile")], 0.04);
        family.observe(&[("stage", "compile")], 0.08);
        family.observe(&[("stage", "compile")], 0.60);
        let mut out = String::new();
        family.render(&mut out);
        assert!(out.contains("# HELP test_hist_seconds Test hist help"));
        assert!(out.contains("# TYPE test_hist_seconds histogram"));
        assert!(out.contains("test_hist_seconds_bucket{le=\"0.05\",stage=\"compile\"} 1"));
        assert!(out.contains("test_hist_seconds_bucket{le=\"0.1\",stage=\"compile\"} 2"));
        assert!(out.contains("test_hist_seconds_bucket{le=\"+Inf\",stage=\"compile\"} 3"));
        assert!(out.contains("test_hist_seconds_count{stage=\"compile\"} 3"));
    }
}

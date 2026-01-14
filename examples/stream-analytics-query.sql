-- Azure Stream Analytics Query
-- Routes OTLP telemetry events by signal_type to appropriate Parquet outputs

-- Route logs to logsoutput
SELECT
    *
INTO
    [logsoutput]
FROM
    [eventhubinput]
WHERE
    signal_type = 'logs'

-- Route traces to tracesoutput
SELECT
    *
INTO
    [tracesoutput]
FROM
    [eventhubinput]
WHERE
    signal_type = 'traces'

-- Route all metrics (gauge, sum, histogram) to metricsoutput
SELECT
    *
INTO
    [metricsoutput]
FROM
    [eventhubinput]
WHERE
    signal_type LIKE 'metrics_%'

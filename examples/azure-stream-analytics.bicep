// Azure Stream Analytics Bicep Template
// Configures complete OTLP ingestion pipeline with Event Hub input and Parquet outputs

@description('Azure region for resources')
param location string = 'westus'

@description('Stream Analytics job name')
param streamAnalyticsJobName string = 'otlp-stream-processor'

@description('Event Hub namespace name')
param eventHubNamespace string = 'otlp-poc-hub-test'

@description('Event Hub name')
param eventHubName string = 'otlp-ingestion'

@description('Event Hub shared access policy name')
param eventHubSharedAccessPolicyName string = 'RootManageSharedAccessKey'

@secure()
@description('Event Hub shared access policy key (secret)')
param eventHubSharedAccessPolicyKey string

@description('Storage account name for outputs')
param storageAccountName string = 'otlppocadls'

@secure()
@description('Storage account key (secret)')
param storageAccountKey string

// Stream Analytics Job
resource streamAnalyticsJob 'Microsoft.StreamAnalytics/streamingjobs@2021-10-01-preview' = {
  name: streamAnalyticsJobName
  location: location
  properties: {
    sku: {
      name: 'Standard'
    }
    outputErrorPolicy: 'Drop'
    eventsOutOfOrderPolicy: 'Adjust'
    eventsOutOfOrderMaxDelayInSeconds: 10
    eventsLateArrivalMaxDelayInSeconds: 5
    dataLocale: 'en-US'
    compatibilityLevel: '1.2'
  }
}

// Input: Event Hub
resource eventhubInput 'Microsoft.StreamAnalytics/streamingjobs/inputs@2021-10-01-preview' = {
  parent: streamAnalyticsJob
  name: 'eventhubinput'
  properties: {
    type: 'Stream'
    datasource: {
      type: 'Microsoft.ServiceBus/EventHub'
      properties: {
        serviceBusNamespace: eventHubNamespace
        eventHubName: eventHubName
        sharedAccessPolicyName: eventHubSharedAccessPolicyName
        sharedAccessPolicyKey: eventHubSharedAccessPolicyKey
        consumerGroupName: '$Default'
      }
    }
    serialization: {
      type: 'Json'
      properties: {
        encoding: 'UTF8'
      }
    }
  }
}

// Output: Logs (Parquet with batching)
resource logsOutput 'Microsoft.StreamAnalytics/streamingjobs/outputs@2021-10-01-preview' = {
  parent: streamAnalyticsJob
  name: 'logsoutput'
  properties: {
    datasource: {
      type: 'Microsoft.Storage/Blob'
      properties: {
        storageAccounts: [
          {
            accountName: storageAccountName
            accountKey: storageAccountKey
          }
        ]
        container: 'logs'
        pathPattern: '{date}/{time}'
        dateFormat: 'yyyy/MM/dd'
        timeFormat: 'HH'
      }
    }
    serialization: {
      type: 'Parquet'
      properties: {}
    }
    timeWindow: '00:05:00'  // 5 minute batching window
    sizeWindow: 2000        // 2000 rows per batch
  }
}

// Output: Traces (Parquet with batching)
resource tracesOutput 'Microsoft.StreamAnalytics/streamingjobs/outputs@2021-10-01-preview' = {
  parent: streamAnalyticsJob
  name: 'tracesoutput'
  properties: {
    datasource: {
      type: 'Microsoft.Storage/Blob'
      properties: {
        storageAccounts: [
          {
            accountName: storageAccountName
            accountKey: storageAccountKey
          }
        ]
        container: 'traces'
        pathPattern: '{date}/{time}'
        dateFormat: 'yyyy/MM/dd'
        timeFormat: 'HH'
      }
    }
    serialization: {
      type: 'Parquet'
      properties: {}
    }
    timeWindow: '00:05:00'
    sizeWindow: 2000
  }
}

// Output: Metrics (Parquet with batching)
resource metricsOutput 'Microsoft.StreamAnalytics/streamingjobs/outputs@2021-10-01-preview' = {
  parent: streamAnalyticsJob
  name: 'metricsoutput'
  properties: {
    datasource: {
      type: 'Microsoft.Storage/Blob'
      properties: {
        storageAccounts: [
          {
            accountName: storageAccountName
            accountKey: storageAccountKey
          }
        ]
        container: 'metrics'
        pathPattern: '{date}/{time}'
        dateFormat: 'yyyy/MM/dd'
        timeFormat: 'HH'
      }
    }
    serialization: {
      type: 'Parquet'
      properties: {}
    }
    timeWindow: '00:05:00'
    sizeWindow: 2000
  }
}

// Transformation: Query that routes events by signal_type
resource transformation 'Microsoft.StreamAnalytics/streamingjobs/transformations@2021-10-01-preview' = {
  parent: streamAnalyticsJob
  name: 'Transformation'
  dependsOn: [
    eventhubInput
    logsOutput
    tracesOutput
    metricsOutput
  ]
  properties: {
    streamingUnits: 1
    query: loadTextContent('./stream-analytics-query.sql')
  }
}

// Outputs
output jobName string = streamAnalyticsJob.name
output jobId string = streamAnalyticsJob.id
output jobState string = streamAnalyticsJob.properties.jobState

// templates/azure/otlp.bicep
// Deploys: Storage Account (ADLS Gen2), Containers, Event Hub Namespace + Hub

param location string = 'westus'
param envName string
param storageAccountName string
param eventHubNamespace string

// Storage Account with ADLS Gen2 enabled
resource storageAccount 'Microsoft.Storage/storageAccounts@2023-01-01' = {
  name: storageAccountName
  location: location
  kind: 'StorageV2'
  sku: {
    name: 'Standard_LRS'
  }
  properties: {
    isHnsEnabled: true  // Enable hierarchical namespace for ADLS Gen2
    minimumTlsVersion: 'TLS1_2'
    allowBlobPublicAccess: false
  }
}

// Blob service (required for containers)
resource blobService 'Microsoft.Storage/storageAccounts/blobServices@2023-01-01' = {
  parent: storageAccount
  name: 'default'
}

// Container: logs
resource logsContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'logs'
  properties: {
    publicAccess: 'None'
  }
}

// Container: traces
resource tracesContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'traces'
  properties: {
    publicAccess: 'None'
  }
}

// Container: metrics-gauge
resource gaugeContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'metrics-gauge'
  properties: {
    publicAccess: 'None'
  }
}

// Container: metrics-sum
resource sumContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'metrics-sum'
  properties: {
    publicAccess: 'None'
  }
}

// Event Hub Namespace
resource eventHubNamespaceResource 'Microsoft.EventHub/namespaces@2023-01-01-preview' = {
  name: eventHubNamespace
  location: location
  sku: {
    name: 'Standard'
    tier: 'Standard'
    capacity: 1
  }
  properties: {
    minimumTlsVersion: '1.2'
  }
}

// Event Hub: otlp-ingestion
resource eventHub 'Microsoft.EventHub/namespaces/eventhubs@2023-01-01-preview' = {
  parent: eventHubNamespaceResource
  name: 'otlp-ingestion'
  properties: {
    partitionCount: 4
    messageRetentionInDays: 1
  }
}

// App Service Plan (Consumption/Dynamic for Functions)
resource appServicePlan 'Microsoft.Web/serverfarms@2023-01-01' = {
  name: 'otlp-${envName}-plan'
  location: location
  sku: {
    name: 'Y1'
    tier: 'Dynamic'
  }
  kind: 'linux'
  properties: {
    reserved: true
  }
}

// Function App - pulls from ghcr.io (public image)
resource functionApp 'Microsoft.Web/sites@2023-01-01' = {
  name: 'otlp-${envName}-func'
  location: location
  kind: 'functionapp,linux,container'
  properties: {
    serverFarmId: appServicePlan.id
    siteConfig: {
      linuxFxVersion: 'DOCKER|ghcr.io/claygorman/otlp2pipeline:latest'
      appSettings: [
        {
          name: 'FUNCTIONS_EXTENSION_VERSION'
          value: '~4'
        }
        {
          name: 'FUNCTIONS_WORKER_RUNTIME'
          value: 'custom'
        }
        {
          name: 'WEBSITES_ENABLE_APP_SERVICE_STORAGE'
          value: 'false'
        }
        {
          name: 'DOCKER_REGISTRY_SERVER_URL'
          value: 'https://ghcr.io'
        }
      ]
    }
    httpsOnly: true
  }
}

// Outputs
output storageAccountId string = storageAccount.id
output storageAccountName string = storageAccount.name
output eventHubNamespaceId string = eventHubNamespaceResource.id
output eventHubName string = eventHub.name
output functionAppName string = functionApp.name
output functionAppUrl string = 'https://${functionApp.properties.defaultHostName}'

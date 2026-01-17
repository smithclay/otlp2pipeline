// templates/azure/otlp.bicep
// Deploys: Storage Account (ADLS Gen2), Containers, Event Hub Namespace + Hub

param location string = 'westus'
param envName string
param storageAccountName string
param eventHubNamespace string
param containerImage string = 'ghcr.io/smithclay/otlp2pipeline:v0.3.0-rc1-amd64'

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

// Container Apps Environment
resource containerAppEnv 'Microsoft.App/managedEnvironments@2023-05-01' = {
  name: 'otlp-${envName}-env'
  location: location
  properties: {
    zoneRedundant: false
  }
}

// Container App - pulls from ghcr.io (public image)
resource containerApp 'Microsoft.App/containerApps@2023-05-01' = {
  name: 'otlp-${envName}-app'
  location: location
  properties: {
    managedEnvironmentId: containerAppEnv.id
    configuration: {
      ingress: {
        external: true
        targetPort: 80
        transport: 'http'
        allowInsecure: false
      }
    }
    template: {
      containers: [
        {
          name: 'otlp2pipeline'
          image: containerImage
          resources: {
            cpu: json('0.5')
            memory: '1Gi'
          }
          env: [
            {
              name: 'EVENTHUB_CONNECTION_STRING'
              value: listKeys(resourceId('Microsoft.EventHub/namespaces/authorizationRules', eventHubNamespace, 'RootManageSharedAccessKey'), '2023-01-01-preview').primaryConnectionString
            }
            {
              name: 'EVENTHUB_NAME'
              value: 'otlp-ingestion'
            }
          ]
        }
      ]
      scale: {
        minReplicas: 1
        maxReplicas: 10
      }
    }
  }
}

// Outputs
output storageAccountId string = storageAccount.id
output storageAccountName string = storageAccount.name
output eventHubNamespaceId string = eventHubNamespaceResource.id
output eventHubName string = eventHub.name
output containerAppName string = containerApp.name
output containerAppUrl string = 'https://${containerApp.properties.configuration.ingress.fqdn}'

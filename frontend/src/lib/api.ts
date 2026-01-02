/**
 * Frostbit API client
 */

export interface Service {
  name: string;
  has_logs: boolean;
  has_traces: boolean;
}

export async function fetchServices(workerUrl: string): Promise<Service[]> {
  const url = `${workerUrl}/v1/services`;

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch services: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();
  return data as Service[];
}

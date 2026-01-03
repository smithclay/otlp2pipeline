import { useState } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/Layout';
import { SetupModal } from './components/SetupModal';
import { ServiceList } from './pages/ServiceList';
import { ServiceDetail } from './pages/ServiceDetail';
import { useCredentials } from './hooks/useCredentials';

export function App() {
  const { credentials, isConfigured, setCredentials } = useCredentials();
  const [showSettings, setShowSettings] = useState(!isConfigured);

  const handleOpenSettings = () => setShowSettings(true);
  const handleCloseSettings = () => setShowSettings(false);

  return (
    <BrowserRouter future={{ v7_relativeSplatPath: true, v7_startTransition: true }}>
      {showSettings && (
        <SetupModal
          onSave={(creds) => {
            setCredentials(creds);
            setShowSettings(false);
          }}
          onClose={isConfigured ? handleCloseSettings : undefined}
          initialValues={credentials ?? undefined}
        />
      )}
      <Layout onOpenSettings={handleOpenSettings}>
        <Routes>
          <Route path="/" element={<ServiceList />} />
          <Route path="/services/:name" element={<ServiceDetail />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}

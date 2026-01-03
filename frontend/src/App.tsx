import { useState } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/Layout';
import { SetupModal } from './components/SetupModal';
import { Home } from './pages/Home';
import { RecordsExplorer } from './pages/RecordsExplorer';
import { Settings } from './pages/Settings';
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
          <Route path="/" element={<Home />} />
          <Route path="/records" element={<RecordsExplorer />} />
          <Route path="/settings" element={<Settings />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}

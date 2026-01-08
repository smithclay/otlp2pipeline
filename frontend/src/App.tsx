import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Layout } from './components/Layout';
import { Home } from './pages/Home';
import { QueryExplorer } from './pages/QueryExplorer';
import { Settings } from './pages/Settings';
import { ToastProvider } from './components/Toast';
import { useCredentials } from './hooks/useCredentials';

export function App() {
  const { isConfigured } = useCredentials();

  return (
    <BrowserRouter future={{ v7_relativeSplatPath: true, v7_startTransition: true }}>
      <ToastProvider>
        <Layout>
          <Routes>
            {/* Redirect to settings if not configured */}
            {!isConfigured && (
              <Route path="*" element={<Navigate to="/settings" replace />} />
            )}
            <Route path="/" element={<Home />} />
            <Route path="/query" element={<QueryExplorer />} />
            <Route path="/settings" element={<Settings />} />
          </Routes>
        </Layout>
      </ToastProvider>
    </BrowserRouter>
  );
}

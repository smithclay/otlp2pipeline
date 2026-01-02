import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/Layout';
import { SetupModal } from './components/SetupModal';
import { ServiceList } from './pages/ServiceList';
import { ServiceDetail } from './pages/ServiceDetail';
import { useCredentials } from './hooks/useCredentials';

export function App() {
  const { isConfigured, setCredentials } = useCredentials();

  return (
    <BrowserRouter>
      {!isConfigured && <SetupModal onSave={setCredentials} />}
      <Layout>
        <Routes>
          <Route path="/" element={<ServiceList />} />
          <Route path="/services/:name" element={<ServiceDetail />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}

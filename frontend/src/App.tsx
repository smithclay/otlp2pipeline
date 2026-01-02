import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/Layout';
import { ServiceList } from './pages/ServiceList';
import { ServiceDetail } from './pages/ServiceDetail';

export function App() {
  return (
    <BrowserRouter>
      <Layout>
        <Routes>
          <Route path="/" element={<ServiceList />} />
          <Route path="/services/:name" element={<ServiceDetail />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}

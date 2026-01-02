import { useNavigate } from 'react-router-dom';
import { Service } from '../lib/api';

interface ServiceCardProps {
  service: Service;
}

interface SignalDotProps {
  label: string;
  active: boolean;
}

function SignalDot({ label, active }: SignalDotProps) {
  return (
    <span className="flex items-center gap-1.5">
      <span
        className={`inline-block h-2 w-2 rounded-full ${
          active ? 'bg-cyan-500' : 'bg-slate-600'
        }`}
      />
      <span className="text-sm text-slate-400">{label}</span>
    </span>
  );
}

export function ServiceCard({ service }: ServiceCardProps) {
  const navigate = useNavigate();

  const handleClick = () => {
    navigate(`/services/${encodeURIComponent(service.name)}`);
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      className="w-full rounded-lg border border-slate-700 bg-slate-800 p-4 text-left transition-colors hover:border-slate-600 hover:bg-slate-700 focus:outline-none focus:ring-2 focus:ring-cyan-500 focus:ring-offset-2 focus:ring-offset-slate-900"
    >
      <div className="flex items-center justify-between">
        <span className="text-base font-medium text-slate-100">
          {service.name}
        </span>
        <div className="flex items-center gap-4">
          <SignalDot label="Logs" active={service.has_logs} />
          <SignalDot label="Traces" active={service.has_traces} />
        </div>
      </div>
    </button>
  );
}

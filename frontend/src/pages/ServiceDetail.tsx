import { useParams } from 'react-router-dom';

export function ServiceDetail() {
  const { name } = useParams<{ name: string }>();

  return (
    <div className="rounded-lg border border-slate-800 bg-slate-800 p-6">
      <h1 className="text-2xl font-semibold text-slate-100">
        Service: <span className="text-cyan-500">{name}</span>
      </h1>
      <p className="mt-2 text-slate-400">Service details will be displayed here.</p>
    </div>
  );
}

import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Gamepad2, Settings, Disc, Play, Cpu, Monitor, Volume2, CpuIcon } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import './App.css';

// --- Mock Data ---
const MOCK_GAMES = [
  { id: 1, title: 'Final Fantasy X', cover: 'https://images.unsplash.com/photo-1542751371-adc38448a05e?auto=format&fit=crop&q=80&w=400&h=600', region: 'NTSC-U' },
  { id: 2, title: 'Metal Gear Solid 2', cover: 'https://images.unsplash.com/photo-1552820728-8b83bb6b773f?auto=format&fit=crop&q=80&w=400&h=600', region: 'NTSC-U' },
  { id: 3, title: 'Shadow of the Colossus', cover: 'https://images.unsplash.com/photo-1518709268805-4e9042af9f23?auto=format&fit=crop&q=80&w=400&h=600', region: 'PAL' },
  { id: 4, title: 'Gran Turismo 4', cover: 'https://images.unsplash.com/photo-1492144534655-ae79c964c9d7?auto=format&fit=crop&q=80&w=400&h=600', region: 'NTSC-J' },
];

export default function App() {
  const [activeTab, setActiveTab] = useState<'library' | 'settings'>('library');
  const [settingsTab, setSettingsTab] = useState<'graphics' | 'audio' | 'bios'>('bios');
  
  // Backend State
  const [biosPath, setBiosPath] = useState<string>('default_bios.bin (Baked-in)');
  const [emulatorStatus, setEmulatorStatus] = useState<string>('Connecting to Engine...');
  const [logs, setLogs] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);

  // Poll status periodically
  useEffect(() => {
    const fetchStatus = async () => {
      try {
        const status = await invoke<string>('get_status');
        setEmulatorStatus(status);
      } catch (e) {
        console.error(e);
      }
    };
    
    fetchStatus();
    const interval = setInterval(fetchStatus, 1000);
    return () => clearInterval(interval);
  }, []);

  // Listen for SIO Logs
  useEffect(() => {
    const unlisten = listen<string>('sio-log', (event) => {
      setLogs(prev => [`[SIO] ${event.payload}`, ...prev].slice(0, 100));
    });
    return () => {
      unlisten.then(f => f());
    };
  }, []);

  // Continuous execution loop
  useEffect(() => {
    let interval: ReturnType<typeof setInterval>;
    if (isRunning) {
      interval = setInterval(async () => {
        try {
          const result = await invoke<string[]>('run_cpu_batch', { steps: 50000 });
          setLogs(prev => [...result.reverse(), ...prev].slice(0, 100));
        } catch (e) {
          console.error(e);
          setIsRunning(false);
        }
      }, 50); // Run batch every 50ms
    }
    return () => clearInterval(interval);
  }, [isRunning]);

  const addLog = (msg: string) => {
    setLogs(prev => [msg, ...prev].slice(0, 100));
  };

  const handleBootISO = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'PlayStation 2 Disc Image',
          extensions: ['iso', 'bin', 'img', 'chd', 'mdf', 'nrg']
        }]
      });
      
      if (selected) {
        addLog(`Loading ISO from: ${selected}`);
        const result = await invoke<string>('boot_game', { path: selected });
        addLog(`Engine: ${result}`);
      }
    } catch (e) {
      addLog(`Error: ${e}`);
    }
  };

  const handleOverrideBIOS = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'PS2 BIOS File',
          extensions: ['bin', 'rom0']
        }]
      });
      
      if (selected) {
        setBiosPath(selected as string);
        addLog(`Custom BIOS selected: ${selected}. Restart required.`);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const handleStepCPU = async () => {
    try {
      const result = await invoke<string>('step_cpu');
      addLog(result);
    } catch (e) {
      addLog(`Error: ${e}`);
    }
  };

  return (
    <div className="flex h-screen w-screen bg-[#0f0f13] text-slate-200 overflow-hidden font-sans selection:bg-purple-500/30">
      
      {/* Sidebar Navigation */}
      <nav className="w-64 bg-[#18181b] border-r border-white/5 flex flex-col p-4 z-10 shadow-2xl relative">
        <div className="flex items-center gap-3 mb-10 px-2 mt-2">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-purple-600 to-blue-600 flex items-center justify-center shadow-[0_0_15px_rgba(147,51,234,0.5)]">
            <Cpu className="text-white w-6 h-6" />
          </div>
          <div>
            <h1 className="text-xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-purple-400 to-blue-400 tracking-tight">
              EmotionX
            </h1>
            <p className="text-xs text-slate-500 font-medium">PS2 Emulator Core</p>
          </div>
        </div>

        <div className="space-y-2 flex-1">
          <NavItem 
            icon={<Gamepad2 className="w-5 h-5" />} 
            label="Game Library" 
            isActive={activeTab === 'library'} 
            onClick={() => setActiveTab('library')} 
          />
          <NavItem 
            icon={<Settings className="w-5 h-5" />} 
            label="Settings" 
            isActive={activeTab === 'settings'} 
            onClick={() => setActiveTab('settings')} 
          />
        </div>
        
        {/* Status indicator connected to Rust backend */}
        <div className="mt-auto px-4 py-3 rounded-xl bg-white/5 border border-white/5 text-xs text-slate-400 flex flex-col gap-2">
          <div className="flex items-center gap-2 font-medium">
            <span className={`w-2 h-2 rounded-full shadow-[0_0_8px_rgba(34,197,94,0.8)] animate-pulse ${emulatorStatus.includes('Idle') ? 'bg-amber-400' : 'bg-green-500'}`}></span>
            EE Status
          </div>
          <p className="truncate opacity-80">{emulatorStatus}</p>
        </div>
      </nav>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col relative overflow-hidden">
        {/* Subtle Background Glow */}
        <div className="absolute top-0 right-0 w-[500px] h-[500px] bg-purple-600/10 rounded-full blur-[120px] -translate-y-1/2 translate-x-1/2 pointer-events-none" />
        <div className="absolute bottom-0 left-0 w-[400px] h-[400px] bg-blue-600/10 rounded-full blur-[100px] translate-y-1/2 -translate-x-1/2 pointer-events-none" />
        
        <div className="p-10 flex-1 relative z-10 overflow-y-auto">
          <AnimatePresence mode="wait">
            {activeTab === 'library' && (
              <motion.div 
                key="library"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                transition={{ duration: 0.2 }}
                className="h-full flex flex-col"
              >
                <div className="flex justify-between items-end mb-8">
                  <div>
                    <h2 className="text-3xl font-bold tracking-tight text-white mb-2">Library</h2>
                    <p className="text-slate-400">Select a title to boot the Emotion Engine.</p>
                  </div>
                  <div className="flex gap-2">
                    <button 
                      onClick={() => setIsRunning(!isRunning)}
                      className={`flex items-center gap-2 px-4 py-2 border rounded-lg text-sm font-medium transition-colors ${
                        isRunning 
                          ? 'bg-red-500/20 text-red-400 border-red-500/30 hover:bg-red-500/30' 
                          : 'bg-green-500/20 text-green-400 border-green-500/30 hover:bg-green-500/30'
                      }`}
                    >
                      <Play className="w-4 h-4" />
                      {isRunning ? 'Pause' : 'Run'}
                    </button>
                    <button 
                      onClick={handleStepCPU}
                      disabled={isRunning}
                      className="flex items-center gap-2 px-4 py-2 bg-purple-600/20 hover:bg-purple-600/40 text-purple-400 border border-purple-500/30 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
                    >
                      <CpuIcon className="w-4 h-4" />
                      Step CPU
                    </button>
                    <button 
                      onClick={handleBootISO}
                      className="flex items-center gap-2 px-4 py-2 bg-white/5 hover:bg-white/10 border border-white/10 rounded-lg text-sm font-medium transition-colors"
                    >
                      <Disc className="w-4 h-4" />
                      Boot Custom ISO
                    </button>
                  </div>
                </div>

                <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-6 pb-20">
                  {MOCK_GAMES.map((game) => (
                    <motion.div 
                      key={game.id}
                      whileHover={{ scale: 1.05, y: -5 }}
                      whileTap={{ scale: 0.98 }}
                      className="group relative rounded-xl overflow-hidden cursor-pointer aspect-[3/4] bg-slate-800 border border-white/5 shadow-xl"
                    >
                      <img src={game.cover} alt={game.title} className="absolute inset-0 w-full h-full object-cover opacity-80 group-hover:opacity-100 transition-opacity duration-300" />
                      <div className="absolute inset-0 bg-gradient-to-t from-black/90 via-black/40 to-transparent flex flex-col justify-end p-4">
                        <div className="translate-y-4 opacity-0 group-hover:translate-y-0 group-hover:opacity-100 transition-all duration-300">
                          <button className="w-10 h-10 mb-3 rounded-full bg-purple-600 hover:bg-purple-500 flex items-center justify-center text-white shadow-[0_0_15px_rgba(147,51,234,0.6)]">
                            <Play className="w-4 h-4 ml-1" />
                          </button>
                        </div>
                        <h3 className="font-semibold text-white leading-tight">{game.title}</h3>
                        <span className="text-[10px] uppercase font-bold tracking-wider text-slate-400 mt-1">{game.region}</span>
                      </div>
                    </motion.div>
                  ))}
                </div>
              </motion.div>
            )}

            {activeTab === 'settings' && (
              <motion.div 
                key="settings"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                transition={{ duration: 0.2 }}
                className="max-w-4xl mx-auto h-full flex flex-col"
              >
                <div className="mb-8">
                  <h2 className="text-3xl font-bold tracking-tight text-white mb-2">Configuration</h2>
                  <p className="text-slate-400">Tune the emulator parameters for optimal performance.</p>
                </div>

                <div className="flex gap-8 flex-1 pb-20">
                  {/* Settings Sidebar */}
                  <div className="w-48 space-y-1">
                    <SettingsTab label="BIOS & Core" icon={<CpuIcon className="w-4 h-4"/>} isActive={settingsTab === 'bios'} onClick={() => setSettingsTab('bios')} />
                    <SettingsTab label="Graphics" icon={<Monitor className="w-4 h-4"/>} isActive={settingsTab === 'graphics'} onClick={() => setSettingsTab('graphics')} />
                    <SettingsTab label="Audio" icon={<Volume2 className="w-4 h-4"/>} isActive={settingsTab === 'audio'} onClick={() => setSettingsTab('audio')} />
                  </div>

                  {/* Settings Content */}
                  <div className="flex-1 bg-white/5 border border-white/5 rounded-2xl p-8 shadow-xl backdrop-blur-sm">
                    {settingsTab === 'bios' && (
                      <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                        <h3 className="text-xl font-semibold text-white border-b border-white/10 pb-4">BIOS Configuration</h3>
                        
                        <div className="space-y-4">
                          <div className="p-4 rounded-xl border border-purple-500/30 bg-purple-500/10 relative overflow-hidden">
                            <div className="absolute top-0 left-0 w-1 h-full bg-purple-500"></div>
                            <h4 className="font-medium text-purple-300 mb-1">Active BIOS</h4>
                            <p className="text-sm text-slate-300 mb-4">{biosPath}</p>
                            <p className="text-xs text-purple-300/70">The emulator is currently using the baked-in SCPH-90001 v18 USA BIOS. This ensures maximum compatibility out of the box.</p>
                          </div>

                          <div className="pt-4 space-y-3">
                            <label className="block text-sm font-medium text-slate-300">Override BIOS File</label>
                            <div className="flex gap-3">
                              <input 
                                type="text" 
                                readOnly 
                                value={biosPath} 
                                placeholder="Select a .bin file to override..."
                                className="flex-1 bg-black/40 border border-white/10 rounded-lg px-4 py-2 text-sm text-slate-300 focus:outline-none focus:border-purple-500 transition-colors"
                              />
                              <button 
                                onClick={handleOverrideBIOS}
                                className="px-4 py-2 bg-white/10 hover:bg-white/15 border border-white/10 rounded-lg text-sm font-medium transition-colors"
                              >
                                Browse
                              </button>
                            </div>
                            <p className="text-xs text-slate-500">Selecting a custom BIOS will require an emulator restart to take effect.</p>
                          </div>
                        </div>
                      </div>
                    )}
                    
                    {settingsTab === 'graphics' && (
                      <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300 text-slate-400">
                        <h3 className="text-xl font-semibold text-white border-b border-white/10 pb-4">Graphics Engine (GS)</h3>
                        <p>Internal Resolution options and rendering backend (Vulkan/DX12) configuration will go here.</p>
                      </div>
                    )}
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* Real-time Dev Console Overlay */}
        <div className="absolute bottom-0 left-0 right-0 h-40 bg-black/80 border-t border-white/10 backdrop-blur-md p-4 overflow-y-auto font-mono text-xs z-50">
          <div className="text-purple-400 mb-2 border-b border-white/10 pb-1 font-semibold flex justify-between">
            <span>EmotionX Engine Output</span>
            <span className="text-slate-500">v0.1.0</span>
          </div>
          {logs.length === 0 ? (
            <div className="text-slate-600 italic">Waiting for engine events...</div>
          ) : (
            <div className="space-y-1">
              {logs.map((log, i) => (
                <div key={i} className={log.startsWith('[SIO]') ? "text-green-400" : "text-slate-300"}><span className="text-slate-500 mr-2">{`>`}</span>{log}</div>
              ))}
            </div>
          )}
        </div>

      </main>
    </div>
  );
}

function NavItem({ icon, label, isActive, onClick }: { icon: React.ReactNode, label: string, isActive: boolean, onClick: () => void }) {
  return (
    <button 
      onClick={onClick}
      className={`w-full flex items-center gap-3 px-4 py-3 rounded-xl transition-all duration-200 ${
        isActive 
          ? 'bg-purple-600/10 text-purple-400 font-medium' 
          : 'text-slate-400 hover:bg-white/5 hover:text-slate-200'
      }`}
    >
      <span className={isActive ? 'text-purple-500' : 'text-slate-500'}>{icon}</span>
      {label}
      {isActive && (
        <motion.div layoutId="nav-indicator" className="absolute left-0 w-1 h-8 bg-purple-500 rounded-r-full" />
      )}
    </button>
  );
}

function SettingsTab({ label, icon, isActive, onClick }: { label: string, icon: React.ReactNode, isActive: boolean, onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-3 px-4 py-2.5 rounded-lg text-sm transition-all ${
        isActive 
          ? 'bg-white/10 text-white font-medium' 
          : 'text-slate-400 hover:bg-white/5 hover:text-slate-200'
      }`}
    >
      <span className={isActive ? 'text-purple-400' : 'text-slate-500'}>{icon}</span>
      {label}
    </button>
  );
}

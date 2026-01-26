import React, { useState, useEffect, useRef } from 'react';
import Sidebar from './components/Sidebar';
import Header from './components/Header';
import WelcomeScreen from './components/WelcomeScreen';
import DashboardScreen from './components/DashboardScreen';
import DeviceSelectionScreen from './components/DeviceSelectionScreen';
import WipeAdvisorScreen from './components/WipeAdvisorScreen';
import WipeProgress from './components/WipeProgress';
import './App.css';
import 'material-design-icons-iconfont/dist/material-design-icons.css';




function App() {
  // All state and ref declarations must come first
  const [history, setHistory] = useState([]);
  const [systemHealth, setSystemHealth] = useState({ health: '...', update_available: false });
  const [securityStatus, setSecurityStatus] = useState({ status: '...', protections_active: false });
  const [scannedOnce, setScannedOnce] = useState(false);
  const [dashboardLoading, setDashboardLoading] = useState(false);
  const [currentPage, setCurrentPage] = useState('welcome');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [selectedDevices, setSelectedDevices] = useState([]);
  const [loading, setLoading] = useState(false);
  const [wipingProgress, setWipingProgress] = useState(0);
  const [chatOpen, setChatOpen] = useState(false);
  const [chatMessages, setChatMessages] = useState([
    { id: 1, text: "Hello! I'm your SecureWipe assistant. How can I help you today?", sender: 'bot' }
  ]);
  const [chatInput, setChatInput] = useState('');
  const [typing, setTyping] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [devices, setDevices] = useState([]);
  const chatMessagesRef = useRef(null);

  // Scroll chat to bottom when messages change
  useEffect(() => {
    if (chatMessagesRef.current) {
      chatMessagesRef.current.scrollTop = chatMessagesRef.current.scrollHeight;
    }
  }, [chatMessages, typing]);
  const handleStartWipe = async () => {
    setLoading(true);
    setWipingProgress(0);
    try {
      const res = await fetch('http://127.0.0.1:8080/api/wipe/start', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ device_ids: selectedDevices, method: 'auto' })
      });
      if (!res.ok) throw new Error('Failed to start wipe');
      // Optionally, poll progress here
      let progress = 0;
      const interval = setInterval(() => {
        progress += 5;
        setWipingProgress(progress);
        if (progress >= 100) {
          clearInterval(interval);
          setLoading(false);
          // Always fetch latest dashboard/device data from backend after wipe
          fetchDashboardData().then(() => {
            setSelectedDevices([]); // Reset selected devices after wipe
            setScannedOnce(true);
            setCurrentPage('dashboard');
          });
        }
      }, 200);
    } catch (e) {
      setLoading(false);
      alert('Failed to start wipe: ' + e);
    }
  };
  // Fetch all dashboard data from backend
  const fetchDashboardData = async () => {
    setDashboardLoading(true);
    try {
      const [devRes, histRes, healthRes, secRes] = await Promise.all([
        fetch('http://127.0.0.1:8080/api/devices'),
        fetch('http://127.0.0.1:8080/api/wipe/history'),
        fetch('http://127.0.0.1:8080/api/system/health'),
        fetch('http://127.0.0.1:8080/api/system/security'),
      ]);
      setDevices(devRes.ok ? await devRes.json() : []);
      setHistory(histRes.ok ? await histRes.json() : []);
      setSystemHealth(healthRes.ok ? await healthRes.json() : { health: 'Unknown', update_available: false });
      setSecurityStatus(secRes.ok ? await secRes.json() : { status: 'Unknown', protections_active: false });
    } catch (e) {
      setDevices([]);
      setHistory([]);
      setSystemHealth({ health: 'Unknown', update_available: false });
      setSecurityStatus({ status: 'Unknown', protections_active: false });
      console.error('Dashboard data fetch failed:', e);
    } finally {
      setDashboardLoading(false);
    }
  };

  const handleStartScan = async () => {
    setScanning(true);
    // Log scan event to backend FIRST
    try {
      await fetch('http://127.0.0.1:8080/api/scan/log', { method: 'POST' });
    } catch (e) {
      // Ignore scan log errors
    }
    // Now fetch dashboard/device data
    if (typeof fetchDashboardData === 'function') {
      await fetchDashboardData();
    }
    setScanning(false);
    if (typeof setScannedOnce === 'function') setScannedOnce(true);
  };

  const handleDeviceSelection = (deviceId) => {
    setSelectedDevices(prev =>
      prev.includes(deviceId) ? prev.filter(id => id !== deviceId) : [...prev, deviceId]
    );
  };

  const handleSendMessage = async () => {
    if (!chatInput.trim()) return;
    setChatMessages(prev => [
      ...prev,
      { id: prev.length + 1, text: chatInput, sender: 'user' }
    ]);
    setChatInput('');
    setTyping(true);
    try {
      const res = await fetch('http://127.0.0.1:8080/api/chatbot', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: chatInput })
      });
      const data = await res.json();
      setChatMessages(prev => [
        ...prev,
        { id: prev.length + 1, text: data.reply || '[No response]', sender: 'bot' }
      ]);
    } catch (e) {
      setChatMessages(prev => [
        ...prev,
        { id: prev.length + 1, text: '[Error: Could not reach chatbot]', sender: 'bot' }
      ]);
    } finally {
      setTyping(false);
    }
  };

  const toggleSidebar = () => {
    setSidebarOpen(!sidebarOpen);
  };

  const handlePageChange = (page) => {
    setCurrentPage(page);
    setSidebarOpen(false);
  };
  // (Removed duplicate state and ref declarations)

  // ...other state and hooks...

  // Place the correct useEffect for advisor fetching here if needed, and all other hooks

  return (
    <div className="app-container">
      <Sidebar 
        currentPage={currentPage} 
        onPageChange={handlePageChange}
        isOpen={sidebarOpen}
      />
      <div className="main-content">
        <Header 
          currentPage={currentPage}
          onMenuToggle={toggleSidebar}
        />
        <div className="content">
          {loading ? (
            <WipeProgress progress={wipingProgress} />
          ) : (
            <>
              {currentPage === 'welcome' && <WelcomeScreen onGetStarted={() => handlePageChange('devices')} />}
              {currentPage === 'dashboard' && (
                <DashboardScreen
                  key={history.length > 0 ? `${history.length}-${history[history.length-1].timestamp}` : 'empty'}
                  devices={devices}
                  history={history}
                  systemHealth={systemHealth}
                  securityStatus={securityStatus}
                  scanning={scanning}
                  scannedOnce={scannedOnce}
                />
              )}
              {currentPage === 'devices' && (
                <DeviceSelectionScreen 
                  devices={devices}
                  scanning={scanning}
                  selectedDevices={selectedDevices}
                  onDeviceSelection={handleDeviceSelection}
                  onStartScan={handleStartScan}
                  onProceed={() => handlePageChange('advisor')}
                />
              )}
              {currentPage === 'advisor' && (
                <WipeAdvisorScreen 
                  selectedDevices={selectedDevices}
                  devices={devices}
                  onStartWipe={handleStartWipe}
                />
              )}
            </>
          )}
        </div>
      </div>
      <div className={`overlay ${sidebarOpen ? 'active' : ''}`} onClick={toggleSidebar}></div>
      {/* Chatbot */}
      <button className="chat-button" onClick={() => setChatOpen(!chatOpen)}>
        <span className="material-icons">chat</span>
      </button>
      <div className={`chat-modal ${chatOpen ? 'active' : ''}`}>
        <div className="chat-header">
          <div className="chat-title">SecureWipe Assistant</div>
          <button className="chat-close" onClick={() => setChatOpen(false)}>
            <span className="material-icons">close</span>
          </button>
        </div>
        <div className="chat-messages" ref={chatMessagesRef}>
          {chatMessages.map(message => (
            <div key={message.id} className={`chat-message ${message.sender}`}>
              {message.text}
            </div>
          ))}
          {typing && (
            <div className="typing-indicator">
              <div className="typing-dot"></div>
              <div className="typing-dot"></div>
              <div className="typing-dot"></div>
            </div>
          )}
        </div>
        <div className="chat-input-container">
          <input 
            type="text" 
            className="chat-input" 
            placeholder="Ask a question..."
            value={chatInput}
            onChange={e => setChatInput(e.target.value)}
            onKeyPress={e => e.key === 'Enter' && handleSendMessage()}
          />
          <button className="chat-send" onClick={handleSendMessage}>
            <span className="material-icons">send</span>
          </button>
        </div>
      </div>
    </div>
  );
}

export default App;

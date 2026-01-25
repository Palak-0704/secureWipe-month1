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

  useEffect(() => {
    if (chatMessagesRef.current) {
      chatMessagesRef.current.scrollTop = chatMessagesRef.current.scrollHeight;
    }
  }, [chatMessages]);

  // Fetch devices from backend
  const fetchDevices = async () => {
    try {
      const res = await fetch('http://127.0.0.1:8080/api/devices');
      if (!res.ok) throw new Error('Failed to fetch devices');
      const data = await res.json();
      setDevices(data);
    } catch (e) {
      setDevices([]);
      console.error('Device fetch failed:', e);
    }
  };

  // Fetch devices when opening dashboard only
  useEffect(() => {
    if (currentPage === 'dashboard') {
      fetchDevices();
    }
    if (currentPage === 'devices') {
      setDevices([]); // Clear device list when entering device selection
    }
  }, [currentPage]);

  const handlePageChange = (page) => {
    setCurrentPage(page);
    setSidebarOpen(false);
  };

  const toggleSidebar = () => {
    setSidebarOpen(!sidebarOpen);
  };

  const handleDeviceSelection = (deviceId) => {
    setSelectedDevices(prev =>
      prev.includes(deviceId) ? prev.filter(id => id !== deviceId) : [...prev, deviceId]
    );
  };

  const handleStartScan = async () => {
    setScanning(true);
    await fetchDevices();
    setScanning(false);
  };

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
          handlePageChange('dashboard');
        }
      }, 200);
    } catch (e) {
      setLoading(false);
      alert('Failed to start wipe: ' + e);
    }
  };

  // Advisor integration: fetch recommendation when entering advisor page
  const [advisor, setAdvisor] = useState(null);
  const [compliance, setCompliance] = useState('');
  useEffect(() => {
    const fetchAdvisor = async () => {
      if (currentPage === 'advisor' && selectedDevices.length > 0) {
        try {
          const res = await fetch('http://127.0.0.1:8080/api/advisor/recommend', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ device_ids: selectedDevices, compliance })
          });
          const data = await res.json();
          setAdvisor(data);
        } catch (e) {
          setAdvisor({ recommendation: 'unknown', rationale: 'Error contacting advisor' });
        }
      } else {
        setAdvisor(null);
      }
    };
    fetchAdvisor();
  }, [currentPage, selectedDevices, compliance]);

  const handleSendMessage = async () => {
    if (chatInput.trim() === '') return;
    const userMessage = {
      id: chatMessages.length + 1,
      text: chatInput,
      sender: 'user'
    };
    setChatMessages([...chatMessages, userMessage]);
    setChatInput('');
    setTyping(true);
    try {
      const res = await fetch('http://127.0.0.1:8080/api/chatbot', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: chatInput, concise: true })
      });
      const data = await res.json();
      setTyping(false);
      setChatMessages(prev => [...prev, {
        id: prev.length + 1,
        text: data.reply || '[No response]',
        sender: 'bot'
      }]);
    } catch (e) {
      setTyping(false);
      setChatMessages(prev => [...prev, {
        id: prev.length + 1,
        text: '[Error contacting chatbot]',
        sender: 'bot'
      }]);
    }
  };



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
              {currentPage === 'welcome' && <WelcomeScreen onGetStarted={() => handlePageChange('dashboard')} />}
              {currentPage === 'dashboard' && <DashboardScreen devices={devices} />}
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

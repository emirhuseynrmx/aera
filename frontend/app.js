const {
  useState,
  useRef,
  useEffect,
  useMemo
} = React;

// API Base
const API = (() => {
  try {
    return window.location.origin === 'null' ? 'http://localhost:3000' : '';
  } catch {
    return '';
  }
})();

const getAuthHeaders = (isJson = true) => {
  const h = {};
  if (isJson) h['Content-Type'] = 'application/json';
  const ak = localStorage.getItem('aera_api_key');
  if (ak) h['x-api-key'] = ak;
  return h;
};

// Parsers
function extractMetrics(text) {
  const m = text.match(/\[AERA_METRICS\]([\s\S]*?)\[\/AERA_METRICS\]/);
  if (!m) return null;
  try {
    const d = JSON.parse(m[1]);
    return {
      net: d.net_nakit != null ? d.net_nakit.toFixed(2) : null,
      risk: d.risk || null,
      burn: d.burn_rate != null ? d.burn_rate.toFixed(2) : null,
      runway: d.runway_ay != null ? d.runway_ay >= 999 ? '∞' : d.runway_ay.toFixed(1) : null,
      healthSkor: d.health_skor ?? null,
      healthHarf: d.health_harf || null,
      healthEmoji: d.health_emoji || null
    };
  } catch {
    return null;
  }
}
function extractIncentives(text) {
  const m = text.match(/\[AERA_INCENTIVES\]([\s\S]*?)\[\/AERA_INCENTIVES\]/);
  if (!m) return null;
  try {
    return JSON.parse(m[1]);
  } catch {
    return null;
  }
}
function extractCashflow(text) {
  const m = text.match(/\[AERA_CASHFLOW\]([\s\S]*?)\[\/AERA_CASHFLOW\]/);
  if (!m) return null;
  try { return JSON.parse(m[1]); } catch { return null; }
}
function stripBlocks(text) {
  return text
    .replace(/\[AERA_METRICS\][\s\S]*?\[\/AERA_METRICS\]/, '')
    .replace(/\[AERA_INCENTIVES\][\s\S]*?\[\/AERA_INCENTIVES\]/, '')
    .replace(/\[AERA_CASHFLOW\][\s\S]*?\[\/AERA_CASHFLOW\]/, '')
    .trim();
}

// Formatters
function fmtTL(val) {
  if (val == null) return '—';
  const n = parseFloat(val);
  if (isNaN(n)) return String(val);
  return n.toLocaleString('tr-TR', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 0
  });
}
function scoreColor(s) {
  if (s >= 80) return 'var(--green)';
  if (s >= 60) return 'var(--yellow)';
  if (s >= 40) return 'var(--orange)';
  return 'var(--red)';
}
function riskColor(r) {
  if (!r) return 'var(--muted)';
  if (r === 'KRİTİK') return 'var(--red)';
  if (r === 'YÜKSEK') return 'var(--orange)';
  if (r === 'ORTA') return 'var(--yellow)';
  return 'var(--green)';
}
function riskEmoji(r) {
  if (!r) return '—';
  const m = {
    'KRİTİK': '🔴',
    'YÜKSEK': '🟠',
    'ORTA': '🟡',
    'DÜŞÜK': '🟢'
  };
  return `${m[r] || ''} ${r}`;
}
function toolLabel(t) {
  return {
    analyze_cash_flow: 'Nakit Analizi',
    get_data_summary: 'Veri Özeti',
    search_incentives: 'Teşvik Arama',
    predict_cashflow: 'Projeksiyon',
    get_health_score: 'Sağlık Skoru',
    compare_sector_benchmark: 'Sektör Kıyası',
    analyze_expense_categories: 'Kategori Analizi',
    detect_anomalies: 'Anomali Tespiti',
    simulate_scenario: 'What-If Simülasyon',
    detect_cash_crunch: 'Nakit Sıkışıklığı'
  }[t] || t;
}
function renderMarkdown(text) {
  // Markdown parse & XSS filtreleme
  var safe = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  safe = safe.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
  safe = safe.replace(/\*(.*?)\*/g, '<em>$1</em>');
  safe = safe.replace(/^#{1,3} (.+)$/gm, '<strong style="font-size:1.04em">$1</strong>');
  safe = safe.replace(/\n\* /g, '\n• ').replace(/\n- /g, '\n• ');
  safe = safe.replace(/\n/g, '<br/>');
  // Ek güvenlik (Script injection koruması)
  safe = safe.replace(/on\w+\s*=/gi, '').replace(/javascript:/gi, '');
  return safe;
}

// Animated count-up
function useCountUp(target, duration = 1400) {
  const [val, setVal] = useState(0);
  const prev = useRef(0);
  useEffect(() => {
    if (target == null) return;
    const from = prev.current,
      to = Number(target);
    const start = performance.now();
    const tick = now => {
      const t = Math.min((now - start) / duration, 1);
      const ease = 1 - Math.pow(1 - t, 3);
      setVal(Math.round(from + (to - from) * ease));
      if (t < 1) requestAnimationFrame(tick);else prev.current = to;
    };
    requestAnimationFrame(tick);
  }, [target, duration]);
  return val;
}

// Inline AERA Logo SVG (fallback)
function AeraLogoMark({
  size = 44
}) {
  return /*#__PURE__*/React.createElement("svg", {
    width: size,
    height: size,
    viewBox: "0 0 100 100",
    fill: "none",
    style: {
      display: 'block'
    }
  }, /*#__PURE__*/React.createElement("defs", null, /*#__PURE__*/React.createElement("linearGradient", {
    id: "lgs",
    x1: "0",
    y1: "1",
    x2: "1",
    y2: "0"
  }, /*#__PURE__*/React.createElement("stop", {
    offset: "0%",
    stopColor: "#00494F",
    stopOpacity: "0.1"
  }), /*#__PURE__*/React.createElement("stop", {
    offset: "30%",
    stopColor: "#00BCD4"
  }), /*#__PURE__*/React.createElement("stop", {
    offset: "100%",
    stopColor: "#00F5FF"
  })), /*#__PURE__*/React.createElement("linearGradient", {
    id: "lgm",
    x1: "0.3",
    y1: "0",
    x2: "0.7",
    y2: "1"
  }, /*#__PURE__*/React.createElement("stop", {
    offset: "0%",
    stopColor: "#6E8094"
  }), /*#__PURE__*/React.createElement("stop", {
    offset: "50%",
    stopColor: "#A8BCC8"
  }), /*#__PURE__*/React.createElement("stop", {
    offset: "100%",
    stopColor: "#6E8094"
  }))), /*#__PURE__*/React.createElement("path", {
    d: "M50 6 L94 94 H6 Z",
    stroke: "url(#lgm)",
    strokeWidth: "2.8",
    strokeLinejoin: "round",
    fill: "none",
    opacity: "0.55"
  }), /*#__PURE__*/React.createElement("line", {
    x1: "28",
    y1: "64",
    x2: "72",
    y2: "64",
    stroke: "url(#lgm)",
    strokeWidth: "1.8",
    opacity: "0.2"
  }), /*#__PURE__*/React.createElement("path", {
    d: "M16 86 C26 70 38 52 48 40 C54 32 60 26 70 18 L68 21 C62 27 56 34 50 42 C40 56 28 74 20 84 Z",
    fill: "url(#lgs)"
  }), /*#__PURE__*/React.createElement("path", {
    d: "M18 84 C28 68 42 48 52 36 C58 28 64 22 74 16",
    stroke: "#00F5FF",
    strokeWidth: "1.8",
    strokeLinecap: "round",
    fill: "none",
    opacity: "0.55"
  }), /*#__PURE__*/React.createElement("path", {
    d: "M22 80 C30 68 40 54 48 44 C52 38 56 34 62 28",
    stroke: "#00F5FF",
    strokeWidth: "0.8",
    strokeLinecap: "round",
    fill: "none",
    opacity: "0.2"
  }));
}
function LogoImg({
  src,
  size,
  className,
  fallbackSize
}) {
  const [err, setErr] = useState(false);
  if (err) return /*#__PURE__*/React.createElement(AeraLogoMark, {
    size: fallbackSize || size || 44
  });
  return /*#__PURE__*/React.createElement("img", {
    src: src,
    alt: "AERA",
    className: className,
    style: size ? {
      height: size,
      width: 'auto'
    } : undefined,
    onError: () => setErr(true)
  });
}

// Error Boundary
class ErrorBoundary extends React.Component {
  constructor(p) {
    super(p);
    this.state = {
      error: null
    };
  }
  static getDerivedStateFromError(e) {
    return {
      error: e.message
    };
  }
  render() {
    if (this.state.error) return /*#__PURE__*/React.createElement("div", {
      className: "err-screen"
    }, /*#__PURE__*/React.createElement("div", {
      style: {
        fontSize: 52,
        marginBottom: 14
      }
    }, "\u26A0"), /*#__PURE__*/React.createElement("div", {
      style: {
        fontSize: 18,
        fontWeight: 700,
        marginBottom: 8
      }
    }, "Sistem Hatas\u0131"), /*#__PURE__*/React.createElement("div", {
      style: {
        fontSize: 12,
        opacity: .5,
        maxWidth: 400,
        textAlign: 'center',
        marginBottom: 22
      }
    }, this.state.error), /*#__PURE__*/React.createElement("button", {
      className: "lp-cta",
      onClick: () => window.location.reload()
    }, "Yeniden Ba\u015Flat"));
    return this.props.children;
  }
}

// SVG Icons
const Icon = ({
  d,
  s = 16
}) => /*#__PURE__*/React.createElement("svg", {
  width: s,
  height: s,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: "1.6",
  strokeLinecap: "round",
  strokeLinejoin: "round"
}, /*#__PURE__*/React.createElement("path", {
  d: d
}));
const IcChat = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"
});
const IcGrid = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z"
});
const IcBuild = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M6 22V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v18M2 22h20M9 8h1M14 8h1M9 12h1M14 12h1"
});
const IcUpload = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12"
});
const IcFile = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6M16 13H8M16 17H8M10 9H8"
});
const IcFlask = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M9 3H15M5 21H19M6.03 12.48L5 21H19L17.97 12.48L14 7H10L6.03 12.48z"
});
const IcSettings = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
});
const IcChartAlt = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M18 20V10 M12 20V4 M6 20v-6 M3 21h18"
});
const IcBank = s => /*#__PURE__*/React.createElement(Icon, {
  s: s,
  d: "M4 10h16v10H4zM2 22h20M12 2L2 7l10 5 10-5-10-5zM6 10v10M10 10v10M14 10v10M18 10v10"
});


// MINI TERMINAL
function MiniTerminal({
  stage
}) {
  const [logs, setLogs] = useState([]);
  const fullLogs = [
    "> initializing autonomous agents...",
    "> analyzing cashflow vectors...",
    "> nakit yakım anomalileri algılanıyor...",
    "> projecting 3-month runway..."
  ];

  useEffect(() => {
    if (stage < 3) return;
    let i = 0;
    const interval = setInterval(() => {
      setLogs(prev => {
        if (prev.length >= fullLogs.length) {
          clearInterval(interval);
          return prev;
        }
        return [...prev, fullLogs[i]];
      });
      i++;
      if (i >= fullLogs.length) clearInterval(interval);
    }, 700);
    return () => clearInterval(interval);
  }, [stage]);

  return /*#__PURE__*/React.createElement("div", {
    className: `mini-term ${stage >= 3 ? 'in' : ''}`
  }, logs.map((log, i) => /*#__PURE__*/React.createElement("div", {
    key: i,
    className: "term-log type-anim"
  }, log)), logs.length < fullLogs.length && stage >= 3 && /*#__PURE__*/React.createElement("div", {
    className: "term-cursor"
  }));
}

// AI TERMINAL OVERLAY
function AITerminalOverlay({ stage }) {
  const [logs, setLogs] = useState([]);
  const fullLogs = [
    "> treasury incentives detected...",
    "> nakit yakım anomalileri tespit edildi...",
    "> forecasting liquidity horizon..."
  ];

  useEffect(() => {
    if (stage < 2) return;
    let i = 0;
    const interval = setInterval(() => {
      setLogs(prev => {
        if (prev.length >= fullLogs.length) {
          clearInterval(interval);
          return prev;
        }
        return [...prev, fullLogs[i]];
      });
      i++;
      if (i >= fullLogs.length) clearInterval(interval);
    }, 1200);
    return () => clearInterval(interval);
  }, [stage]);

  return /*#__PURE__*/React.createElement("div", {
    className: `ai-terminal-overlay ${stage >= 2 ? 'in' : ''}`
  }, logs.map((log, i) => /*#__PURE__*/React.createElement("div", {
    key: i,
    className: "ai-term-line type-anim"
  }, log)), logs.length < fullLogs.length && stage >= 2 && /*#__PURE__*/React.createElement("div", {
    className: "ai-cursor"
  }));
}

// BOOT SCREEN
function BootScreen({ onComplete }) {
  const [text, setText] = useState('Initializing Financial Intelligence...');
  useEffect(() => {
    const t1 = setTimeout(() => setText('Neural network formation...'), 800);
    const t2 = setTimeout(() => setText('Establishing secure agent protocols...'), 1600);
    const t3 = setTimeout(onComplete, 2400);
    return () => { clearTimeout(t1); clearTimeout(t2); clearTimeout(t3); };
  }, [onComplete]);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: 'absolute', inset: 0, background: '#000', zIndex: 9999,
      display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
      fontFamily: 'var(--mono)', color: 'var(--acc)', fontSize: 13
    }
  }, /*#__PURE__*/React.createElement(LogoImg, {
    src: "/assets/aera-logo-transparent.png",
    fallbackSize: 120,
    style: { width: 140, marginBottom: 30, filter: 'drop-shadow(0 0 16px rgba(0,212,229,0.3))', animation: 'pulse2 1.5s infinite' }
  }), /*#__PURE__*/React.createElement("div", { className: "type-anim" }, "> ", text));
}

// LANDING PAGE
function LandingPage({ onEnter }) {
  const [stage, setStage] = useState(0);
  const n1 = useCountUp(stage >= 4 ? 90 : 0, 1800);
  const n2 = useCountUp(stage >= 4 ? 10 : 0, 1600);
  const n3 = useCountUp(stage >= 4 ? 4 : 0, 2000);
  const n4 = useCountUp(stage >= 4 ? 23 : 0, 1400);
  const scrollHow = () => { const el = document.getElementById('lp-how'); if (el) el.scrollIntoView({ behavior: 'smooth' }); };

  useEffect(() => {
    const t1 = setTimeout(() => setStage(1), 300);
    const t2 = setTimeout(() => setStage(2), 1500);
    const t3 = setTimeout(() => setStage(3), 2700);
    const t4 = setTimeout(() => setStage(4), 3900);
    return () => { clearTimeout(t1); clearTimeout(t2); clearTimeout(t3); clearTimeout(t4); };
  }, []);

  const ce = React.createElement;

  // Section 1 — Hero
  const hero = ce('div', { className: 'lp-hero-section' },
    ce('div', { className: 'lp-bg', 'aria-hidden': 'true' },
      ce('div', { className: 'lp-halo' }),
      ce('div', { className: 'lp-orb lp-orb1' }),
      ce('div', { className: 'lp-orb lp-orb2' }),
      ce('div', { className: 'lp-orb lp-orb3' }),
      ce('div', { className: 'lp-aurora' }),
      ce('div', { className: 'lp-aurora2' }),
      ce('div', { className: `lp-glow ${stage >= 1 ? 'on' : ''}` })
    ),
    ce('div', { className: 'lp-hero-split' },
      ce('div', { className: 'lp-hero-left' },
        ce('div', { className: `lp-logo ${stage >= 1 ? 'in' : ''}` },
          ce(LogoImg, { src: '/assets/aera-logo-transparent.png', className: 'lp-logo-img', fallbackSize: 200 })
        ),
        ce('div', { className: 'lp-tag' }, "T\u00dcRK\u0130YE'DEK\u0130 4 M\u0130LYON KOB\u0130 \u0130\u00c7\u0130N OTONOM CFO"),
        ce('h1', { className: 'lp-h1' }, 'İşletmenizin finansal geleceğini 90 saniyede gören', ce('br'), ce('em', null, 'otonom yapay zeka.')),
        ce('p', { className: 'lp-sub' }, 'AERA verinizi okur, riskleri kendi kendine tespit eder, size uygun te\u015fvikleri bulur \u2014 ve her karar\u0131n\u0131n arkas\u0131ndaki mant\u0131\u011f\u0131 size ad\u0131m ad\u0131m g\u00f6sterir.'),
        ce('div', { className: 'lp-btns' },
          ce('button', { className: 'lp-btn-primary', onClick: onEnter }, '\u00dccretsiz Analize Ba\u015fla \u2192'),
          ce('button', { className: 'lp-btn-ghost', onClick: scrollHow }, 'AERA Nas\u0131l D\u00fc\u015f\u00fcn\u00fcr? \u2193')
        )
      ),
      ce('div', { className: 'lp-hero-right' },
        ce('div', { className: `lp-hero-map ${stage >= 1 ? 'in' : ''}` },
          ce('div', { className: 'lp-hero-map-title' }, 'OTONOM MUHAKEME S\u00dcREC\u0130'),
          stage >= 1 && ce('div', { className: 'lp-map-node active done' }, ce('div', { className: 'lp-map-icon' }, '\ud83d\udcca'), 'Veri okundu: son 6 ay nakit ak\u0131\u015f\u0131.'),
          stage >= 2 && ce('div', { className: 'lp-map-node active warn' }, ce('div', { className: 'lp-map-icon' }, '\ud83d\udd0d'), 'Anomali bulundu: giderlerde ani s\u0131\u00e7rama.'),
          stage >= 3 && ce('div', { className: 'lp-map-node active warn', style: { animationDelay: '0s' } }, ce('div', { className: 'lp-map-icon' }, '📉'), ce('span', null, ce('span', { style: { color: 'var(--red)', fontWeight: 'bold' } }, 'Kritik durum: '), 'nakit ömrü 2.3 ay kaldı.')),
          stage >= 4 && ce('div', { className: 'lp-map-node active done' }, ce('div', { className: 'lp-map-icon' }, '\ud83c\udfdb\ufe0f'), 'Te\u015fvik bulundu: KOSGEB \u0130\u015fletme Geli\u015ftirme.')
        )
      )
    )
  );

  // Section 2 — Metrics Strip
  const metrics = ce('div', { className: 'lp-metrics' },
    [{ n: n1 + 'sn', l: 'ANAL\u0130Z S\u00dcRES\u0130', d: 'Veri y\u00fcklemeden ilk rapora' },
     { n: n2, l: 'OTONOM ARA\u00c7', d: 'Gemini destekli analiz motoru' },
     { n: n3 + 'M', l: 'HEDEF KOB\u0130', d: 'T\u00fcrkiye\'deki eri\u015filebilir pazar' },
     { n: n4, l: 'DEMO SENARYOSU', d: 'Restoran, in\u015faat, e-ticaret...' }
    ].map((m, i) => ce('div', { key: i, className: 'lp-met-card' },
      ce('div', { className: 'lp-met-num' }, m.n),
      ce('div', { className: 'lp-met-lbl' }, m.l),
      ce('div', { className: 'lp-met-desc' }, m.d)
    ))
  );

  // Section 3 — How it Works
  const howItWorks = ce('div', { className: 'lp-how', id: 'lp-how' },
    ce('div', { className: 'lp-sec-title' }, 'AERA Nas\u0131l D\u00fc\u015f\u00fcn\u00fcr?'),
    ce('div', { className: 'lp-sec-sub' }, '\u00c7o\u011fu yapay zeka size bir cevap verir. AERA size d\u00fc\u015f\u00fcnce s\u00fcrecini verir.'),
    ce('div', { className: 'lp-steps' },
      [{ num: '1', icon: '\ud83d\udc41\ufe0f', title: 'Alg\u0131lar', desc: 'CSV\'nizi ya da demo senaryonuzu okur, finansal yap\u0131n\u0131z\u0131 saniyeler i\u00e7inde haritalar.' },
       { num: '2', icon: '\u2699\ufe0f', title: 'Muhakeme Eder', desc: '3 ajanl\u0131 pipeline: Planner soruyu par\u00e7alar \u2192 Executor 10 arac\u0131 zincirler \u2192 Critic cevab\u0131 do\u011frular. Her ad\u0131m izlenebilir.' },
       { num: '3', icon: '\ud83d\udca1', title: 'A\u00e7\u0131klar', desc: 'Sadece sonucu de\u011fil, o sonuca nas\u0131l vard\u0131\u011f\u0131n\u0131 g\u00f6sterir. Her karar izlenebilir.' }
      ].map((s, i) => ce('div', { key: i, className: 'lp-step' },
        ce('div', { className: 'lp-step-num' }, s.num),
        ce('div', { className: 'lp-step-title' }, s.title),
        ce('div', { className: 'lp-step-desc' }, s.desc)
      ))
    )
  );

  // Section 4 — Features
  const features = ce('div', { className: 'lp-section' },
    ce('div', { className: 'lp-sec-title' }, 'Yetenekler'),
    ce('div', { className: 'lp-sec-sub' }, 'Otonom analiz motoru hangi sorular\u0131 yan\u0131tlar?'),
    ce('div', { className: 'lp-features' },
      [{ icon: '\ud83d\udcb5', title: 'Maa\u015flar\u0131 \u00e7\u0131karabilir miyim?', desc: 'Nakit ak\u0131\u015f\u0131 projeksiyonu \u00f6n\u00fcm\u00fczdeki 6 ay\u0131 g\u00f6sterir; s\u0131k\u0131\u015f\u0131kl\u0131\u011f\u0131 \u00f6nceden uyar\u0131r.' },
       { icon: '\ud83d\udd14', title: 'Proaktif Risk Alarm\u0131', desc: 'Z-score anomali tespiti harcamalardaki anormal s\u0131\u00e7ramay\u0131 siz fark etmeden yakalar.' },
       { icon: '\ud83c\udfe6', title: 'Te\u015fvik & Hibe E\u015fle\u015ftirme', desc: 'Size uygun KOSGEB/T\u00dcB\u0130TAK programlar\u0131n\u0131 otomatik bulur. Ka\u00e7\u0131rd\u0131\u011f\u0131n\u0131z deste\u011fi bir daha ka\u00e7\u0131rmay\u0131n.' },
       { icon: '\ud83d\udd2e', title: 'What-If Sim\u00fclasyonu', desc: '"2 ki\u015fi alsam? Kira %20 artsa?" \u2014 karar\u0131 vermeden sonucunu g\u00f6r\u00fcn.' }
      ].map((f, i) => ce('div', { key: i, className: 'lp-feat' },
        ce('div', { className: 'lp-feat-icon' }, f.icon),
        ce('div', { className: 'lp-feat-title' }, f.title),
        ce('div', { className: 'lp-feat-desc' }, f.desc)
      ))
    )
  );

  // Section 5 — Comparison
  const comparison = ce('div', { className: 'lp-section' },
    ce('div', { className: 'lp-sec-title' }, 'Neden AeraCFO?'),
    ce('div', { className: 'lp-sec-sub' }, 'Mevcut alternatiflerin yan\u0131nda'),
    ce('div', { className: 'lp-compare' },
      ce('div', { className: 'lp-comp-card' },
        ce('div', { className: 'lp-comp-name' }, 'Excel + Muhasebeci'),
        ce('div', { className: 'lp-comp-sub' }, 'Geleneksel Y\u00f6ntem'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' 2\u20137 g\u00fcn analiz s\u00fcresi'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' 2.000\u20138.000 \u20ba/ay'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' Proaktif uyar\u0131 yok'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' Te\u015fvik takibi manuel')
      ),
      ce('div', { className: 'lp-comp-card win' },
        ce('div', { className: 'lp-comp-name', style: { color: '#00E68C' } }, '\u2726 AeraCFO'),
        ce('div', { className: 'lp-comp-sub', style: { color: 'rgba(0,230,140,.6)' } }, 'AI Finansal Asistan'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-y' }, '\u2713'), ' 90 saniyede tam analiz'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-y' }, '\u2713'), ' \u00dccretsiz ba\u015flang\u0131\u00e7'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-y' }, '\u2713'), ' Otomatik anomali alarm\u0131'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-y' }, '\u2713'), ' KOSGEB/T\u00dcB\u0130TAK e\u015fle\u015ftirme')
      ),
      ce('div', { className: 'lp-comp-card' },
        ce('div', { className: 'lp-comp-name' }, 'Kurumsal ERP'),
        ce('div', { className: 'lp-comp-sub' }, 'SAP / NetSuite / Logo'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' KOB\u0130 i\u00e7in fazla karma\u015f\u0131k'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' 50.000+ \u20ba/y\u0131l lisans'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' 6\u201318 ay kurulum'),
        ce('div', { className: 'lp-comp-item' }, ce('span', { className: 'lp-comp-n' }, '\u2717'), ' Agentic AI yetene\u011fi yok')
      )
    )
  );

  // Section 6 — CTA + Footer
  const cta = ce('div', { className: 'lp-cta-section' },
    ce('div', { className: 'lp-cta-big' }, '\u0130\u015fletmenizin CFO\'su art\u0131k 90 saniye uza\u011f\u0131nda.'),
    ce('button', { className: 'lp-btn-primary', onClick: onEnter, style: { fontSize: 16, padding: '18px 48px' } }, '\u00dccretsiz Analize Ba\u015fla \u2192'),
    ce('div', { className: 'lp-trust' },
      'Rust + Polars + Gemini 2.5 Flash ile geli\u015ftirildi',
      ce('span', { className: 'lp-trust-sep' }),
      'Google Gemini AI Hackathon 2026',
      ce('span', { className: 'lp-trust-sep' }),
      '10 Otonom Analiz Arac\u0131'
    )
  );

  const footer = ce('div', { className: 'lp-footer' },
    ce('div', { className: 'lp-footer-text' },
      'AeraCFO \u2014 Otonom KOB\u0130 Finansal Asistan\u0131',
      ce('br'), 'Rust/Axum \u00b7 Polars \u00b7 Gemini 2.5 Flash \u00b7 3 Agent \u00b7 10 Tool \u00b7 Multi-Step Reasoning'
    )
  );

  return ce('div', { className: 'lp-wrap' }, hero, metrics, howItWorks, features, comparison, cta, footer);
}

// SIDEBAR
const SECTORS = [{
  id: 'restoran', label: 'Restoran'
}, {
  id: 'cafe', label: 'Kafe & F&B'
}, {
  id: 'otomotiv_servis', label: 'Otomotiv Servis'
}, {
  id: 'perakende', label: 'Perakende'
}, {
  id: 'e_ticaret', label: 'E-Ticaret'
}, {
  id: 'ihracat', label: 'İhracat'
}, {
  id: 'teknoloji_startup', label: 'Teknoloji & Startup'
}, {
  id: 'yazilim_ajans', label: 'Yazılım Ajansı'
}, {
  id: 'danismanlik', label: 'Danışmanlık'
}, {
  id: 'muhasebe_buro', label: 'Muhasebe Bürosu'
}, {
  id: 'imalat', label: 'İmalat'
}, {
  id: 'insaat', label: 'İnşaat'
}, {
  id: 'emlak', label: 'Emlak'
}, {
  id: 'tekstil', label: 'Tekstil'
}, {
  id: 'turizm', label: 'Turizm'
}, {
  id: 'egitim_kursu', label: 'Eğitim Kursu'
}, {
  id: 'lojistik', label: 'Lojistik'
}, {
  id: 'medikal', label: 'Medikal'
}, {
  id: 'saglik_klinik', label: 'Sağlık Kliniği'
}, {
  id: 'gida_uretim', label: 'Gıda Üretim'
}, {
  id: 'kobi', label: 'Genel KOBİ'
}, {
  id: 'kuafor_guzellik', label: 'Kuaför & Güzellik'
}, {
  id: 'eczane', label: 'Eczane'
}];
function Sidebar({
  view,
  setView,
  onDemo,
  onUpload,
  uploadStatus,
  fileRef,
  incentives,
  serverOk,
  sessionId,
  onExportPDF,
  onPreviewPDF,
  onSettings,
  loading
}) {
  const sCls = s => ({
    ok: 'st-ok',
    err: 'st-err',
    loading: 'st-load'
  })[s] || '';
  return /*#__PURE__*/React.createElement("aside", {
    className: "sb"
  }, /*#__PURE__*/React.createElement("div", {
    className: "sb-logo"
  }, /*#__PURE__*/React.createElement(LogoImg, {
    src: "/assets/aera-logo-mark.png",
    className: "sb-logo-img",
    fallbackSize: 44
  })), /*#__PURE__*/React.createElement("div", {
    className: "sb-sec"
  }, "MOD\xDCLLER"), [{
    id: 'chat',
    label: 'AI Finansal Asistan',
    icon: IcChat(15)
  }, {
    id: 'dashboard',
    label: 'Finansal Özet Paneli',
    icon: IcGrid(15)
  }, {
    id: 'incentives',
    label: 'Teşvik & Hibe',
    icon: IcBuild(15),
    badge: incentives.length > 0 ? incentives.length : null
  }, {
    id: 'whatif',
    label: 'What-If',
    icon: IcFlask(15)
  }].map(({
    id,
    label,
    icon,
    badge
  }) => /*#__PURE__*/React.createElement("button", {
    key: id,
    className: `sb-nav ${view === id ? 'active' : ''}`,
    onClick: () => setView(id)
  }, icon, " ", /*#__PURE__*/React.createElement("span", null, label), badge != null && /*#__PURE__*/React.createElement("span", {
    className: "sb-badge"
  }, badge))), /*#__PURE__*/React.createElement("div", {
    className: "sb-sec"
  }, "VER\u0130 ENTEGRASYONU"), /*#__PURE__*/React.createElement("button", {
    className: "sb-demo",
    onClick: () => onDemo(null),
    disabled: uploadStatus?.state === 'loading'
  }, /*#__PURE__*/React.createElement("span", {
    className: "sb-demo-ic"
  }, IcFlask(16)), /*#__PURE__*/React.createElement("span", {
    className: "sb-demo-txt"
  }, /*#__PURE__*/React.createElement("strong", null, "\xD6rnek Veri Y\xFCkle"), /*#__PURE__*/React.createElement("small", null, "23 sekt\xF6rel model, rastgele y\xFCkle"))), /*#__PURE__*/React.createElement("div", {
    className: "sb-sec2"
  }, "Sekt\xF6r Se\xE7"), /*#__PURE__*/React.createElement("div", {
    className: "sb-sectors"
  }, SECTORS.map(s => /*#__PURE__*/React.createElement("button", {
    key: s.id,
    className: "sb-sector",
    onClick: () => onDemo(s.id),
    disabled: uploadStatus?.state === 'loading'
  }, /*#__PURE__*/React.createElement("span", {
    className: "sb-dot"
  }), /*#__PURE__*/React.createElement("span", null, s.label)))), /*#__PURE__*/React.createElement("button", {
    className: "sb-upload",
    onClick: () => fileRef.current?.click()
  }, /*#__PURE__*/React.createElement("input", {
    ref: fileRef,
    type: "file",
    accept: ".csv,.xlsx",
    onChange: onUpload,
    style: {
      display: 'none'
    }
  }), IcUpload(14), /*#__PURE__*/React.createElement("span", null, "CSV / Excel Yükle")), uploadStatus && /*#__PURE__*/React.createElement("div", {
    className: `sb-status ${sCls(uploadStatus?.state)}`
  }, uploadStatus.text), /*#__PURE__*/React.createElement("button", {
    className: "sb-pdf",
    style: { color: sessionId && !loading ? 'var(--fg)' : 'var(--muted)', borderColor: 'rgba(255,255,255,0.1)', cursor: sessionId && !loading ? 'pointer' : 'not-allowed', opacity: sessionId && !loading ? 1 : 0.5, marginBottom: '6px' },
    disabled: !sessionId || loading,
    onClick: onPreviewPDF
  }, /*#__PURE__*/React.createElement("i", { className: "ph ph-eye" }), " ", /*#__PURE__*/React.createElement("span", null, "Rapor Önizleme")), /*#__PURE__*/React.createElement("button", {
    className: "sb-pdf",
    style: { color: sessionId && !loading ? 'var(--fg)' : 'var(--muted)', borderColor: 'rgba(255,255,255,0.1)', cursor: sessionId && !loading ? 'pointer' : 'not-allowed', opacity: sessionId && !loading ? 1 : 0.5 },
    disabled: !sessionId || loading,
    onClick: onExportPDF
  }, IcFile(14), " ", /*#__PURE__*/React.createElement("span", null, "Detay Raporu (PDF)")), incentives.length > 0 && /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("div", {
    className: "sb-sec",
    style: {
      marginTop: 14
    }
  }, "FON FIRSATLARI (", incentives.length, ")"), incentives.slice(0, 3).map((inc, i) => /*#__PURE__*/React.createElement("div", {
    key: i,
    className: "sb-inc"
  }, /*#__PURE__*/React.createElement("div", {
    className: "sb-inc-name"
  }, inc.isim), /*#__PURE__*/React.createElement("div", {
    className: "sb-inc-amt"
  }, inc.tutar), /*#__PURE__*/React.createElement("span", {
    className: "sb-inc-badge"
  }, inc.tip))), incentives.length > 3 && /*#__PURE__*/React.createElement("button", {
    className: "sb-sector",
    onClick: () => setView('incentives')
  }, /*#__PURE__*/React.createElement("span", {
    className: "sb-dot",
    style: {
      background: 'var(--acc)'
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      color: 'var(--acc)'
    }
  }, "+", incentives.length - 3, " te\u015Fvik daha \u2192"))),

  /* GELECEK ENTEGRASYONLAR KARTI */
  /*#__PURE__*/React.createElement("div", {
    className: "sb-sec",
    style: { marginTop: 24, borderTop: '1px solid rgba(255,255,255,0.05)', paddingTop: 16 }
  }, "GELECEK ENTEGRASYONLAR"),
  /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '8px',
      background: 'rgba(255,255,255,0.02)', padding: '12px', borderRadius: '8px',
      border: '1px solid rgba(255,255,255,0.04)'
    }
  },
    /*#__PURE__*/React.createElement("div", { style: { display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px', color: 'rgba(255,255,255,0.6)' } }, "🏢", /*#__PURE__*/React.createElement("span", null, "Paraşüt API")),
    /*#__PURE__*/React.createElement("div", { style: { display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px', color: 'rgba(255,255,255,0.6)' } }, "🏢", /*#__PURE__*/React.createElement("span", null, "Logo Yazılım")),
    /*#__PURE__*/React.createElement("div", { style: { display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px', color: 'rgba(255,255,255,0.6)' } }, "🏦", /*#__PURE__*/React.createElement("span", null, "QNB Açık Bankacılık")),
    /*#__PURE__*/React.createElement("div", { style: { fontSize: '10px', color: 'var(--acc)', marginTop: '4px', fontStyle: 'italic' } }, "🔗 Çok yakında")
  ),

  /*#__PURE__*/React.createElement("div", {
    className: "sb-footer"
  }, /*#__PURE__*/React.createElement("div", {
    className: "sb-sys"
  }, /*#__PURE__*/React.createElement("span", {
    className: `sb-sys-dot ${serverOk ? 'on' : 'off'}`
  }), /*#__PURE__*/React.createElement("span", null, serverOk ? 'Motor aktif' : 'Bağlantı yok'))));
}

// METRICS ROW
function MetricsRow({
  metrics
}) {
  if (!metrics) return null;
  return /*#__PURE__*/React.createElement("div", {
    className: "met-row"
  }, [{
    l: 'NET NAKİT',
    v: fmtTL(metrics.net),
    u: '₺',
    c: 'var(--acc)'
  }, {
    l: 'RİSK',
    v: riskEmoji(metrics.risk),
    c: riskColor(metrics?.risk)
  }, {
    l: 'NAKİT YAKIMI',
    v: fmtTL(metrics.burn),
    u: '₺/ay',
    c: 'var(--orange)'
  }, {
    l: 'NAKİT ÖMRÜ',
    v: metrics.runway === '∞' ? '∞' : metrics.runway + ' ay',
    c: metrics.runway === '∞' || parseFloat(metrics.runway) >= 3 ? 'var(--green)' : 'var(--red)'
  }].map((m, i) => /*#__PURE__*/React.createElement("div", {
    key: i,
    className: "met-card"
  }, /*#__PURE__*/React.createElement("div", {
    className: "met-lbl"
  }, m.l), /*#__PURE__*/React.createElement("div", {
    className: "met-val",
    style: {
      color: m.c
    }
  }, m.v, m.u && /*#__PURE__*/React.createElement("span", {
    className: "met-unit"
  }, " ", m.u)))));
}

// CHAT VIEW
const SUGGESTIONS = [{
  label: 'Önümüzdeki 3 ay maaşları ödeyebilir miyim?',
  desc: 'Nakit projeksiyonu ve darboğaz modellemesi',
  icon: '💧',
  iconColor: '#38BDF8',
  cmd: 'Önümüzdeki 3 aylık nakit projeksiyonu ve darboğaz modellemesini oluştur'
}, {
  label: 'Harcamalarda bir anormallik var mı?',
  desc: 'Z-score tabanlı anomali tespiti',
  icon: '🛡️',
  iconColor: '#F472B6',
  cmd: 'Giderlerdeki anormal artışları ve gizli maliyetleri tespit et'
}, {
  label: 'Alabileceğim devlet teşviki var mı?',
  desc: 'KOSGEB & TÜBİTAK eşleştirmesi',
  icon: '🎁',
  iconColor: '#34D399',
  cmd: 'Firmam için uygun devlet hibe ve fon programlarını tara'
}, {
  label: 'Rakiplerime göre ne durumdayım?',
  desc: 'Sektörel benchmark analizi',
  icon: '📊',
  iconColor: '#A78BFA',
  cmd: 'Sektör benchmark verileriyle finansal performans karşılaştırması yap'
}, {
  label: 'Yeni bir yazılımcı işe alırsam ne olur?',
  desc: 'What-If (Eğer) senaryo simülasyonu',
  icon: '⚗️',
  iconColor: '#FBBF24',
  cmd: 'Aylık giderlere 100.000 TL maaş yükü eklersem nakit ömrü nasıl etkilenir?'
}, {
  label: 'Gelirler düşerse ne kadar dayanabilirim?',
  desc: 'Operasyonel dayanıklılık (Nakit Ömrü) testi',
  icon: '📉',
  iconColor: '#FB7185',
  cmd: 'Mevcut nakit yakım hızımla ne kadar süre dayanabilirim?'
}];

// THOUGHT CHAIN COMPONENTS

const TOOL_THOUGHTS = {
  analyze_cash_flow: { text: "Nakit akışınızı okudum — son aylar elimde.", icon: '📊' },
  detect_anomalies: { text: "Dur, harcamalarda anormal bir sıçrama var.", icon: '🔍' },
  get_health_score: { text: "Finansal sağlık skorunuzu hesapladım.", icon: '🏥' },
  predict_cashflow: { text: "Önümüzdeki ayları modelliyorum...", icon: '📈' },
  search_incentives: { text: "Profille uyumlu teşvik programları arıyorum.", icon: '🏛️' },
  compare_sector_benchmark: { text: "Sektör ortalamasıyla karşılaştırıyorum.", icon: '⚖️' },
  detect_cash_crunch: { text: "Nakit sıkışıklığı riski tarıyorum.", icon: '⚠️' },
  simulate_scenario: { text: "Senaryoyu simüle ediyorum...", icon: '🧪' },
  analyze_expense_categories: { text: "Gider kategorilerini analiz ediyorum.", icon: '💸' },
  get_data_summary: { text: "Veri yapısını inceliyorum.", icon: '📂' },
  fallback: { text: "Ek analiz yapıyorum...", icon: '⚙️' }
};

function GhostThoughtMap() {
  return React.createElement("div", { className: "ghost-map" },
    React.createElement("div", { className: "ghost-node", style: { top: '10%', left: '5%' } }, '📂 Veri Yükleme'),
    React.createElement("div", { className: "ghost-node", style: { top: '10%', left: '55%' } }, '🔍 Anomali Tarama'),
    React.createElement("div", { className: "ghost-node", style: { top: '45%', left: '25%' } }, '📊 Nakit Analizi'),
    React.createElement("div", { className: "ghost-node", style: { top: '45%', left: '65%' } }, '🏛️ Teşvik Eşleştirme'),
    React.createElement("div", { className: "ghost-node", style: { top: '80%', left: '38%' } }, '✅ Sonuç'),
    React.createElement("div", { className: "ghost-line", style: { top: '22%', left: '18%', width: '1px', height: '23%' } }),
    React.createElement("div", { className: "ghost-line", style: { top: '22%', left: '68%', width: '1px', height: '23%' } }),
    React.createElement("div", { className: "ghost-line", style: { top: '58%', left: '38%', width: '1px', height: '22%' } }),
    React.createElement("div", { className: "ghost-line", style: { top: '18%', left: '18%', width: '37%', height: '1px' } })
  );
}

function BreathingCore({ status, hasError }) {
  const isThinking = status === 'thinking';
  return React.createElement("div", { className: "core-wrap" },
    !isThinking && !hasError && React.createElement(GhostThoughtMap),
    React.createElement("div", { 
      className: "core-halo", 
      style: { 
        animationDuration: isThinking ? '1.5s' : '4s',
        background: hasError ? 'rgba(113, 113, 122, 0.4)' : undefined
      } 
    }),
    React.createElement("div", { className: "core-halo-ring" }),
    React.createElement(LogoImg, { 
      src: "/assets/aera-logo-mark.png", 
      className: "core-logo", 
      style: isThinking ? { animation: 'coreLogoPulse 1.2s ease-in-out infinite alternate' } : undefined
    }),
    React.createElement("div", { className: "core-title" }, hasError ? "Sistem Yanıt Veremiyor" : "Başlamaya hazırım. Finansal verinizi yükleyin."),
    React.createElement("div", { className: "core-sub" }, hasError ? "Lütfen internet bağlantınızı kontrol edin veya tekrar deneyin." : "Verinizi yükleyin; nakit akışınızı, risklerinizi ve teşvik fırsatlarınızı 90 saniyede analiz edip her bulgumu adım adım göstereyim."),
    React.createElement("div", { className: "core-status" }, isThinking ? "Otonom analiz çalışıyor..." : "")
  );
}

function ThoughtChain({ tools, isError, agentTrace }) {
  // Agent pipeline animasyonu (Planner & Critic fazları varsa)
  const pipelineNodes = (() => {
    if (isError || !agentTrace) return null;
    const nodes = [];
    nodes.push({
      kind: 'planner',
      icon: '🧭',
      text: agentTrace.subtask_count > 0
        ? `Planner: ${agentTrace.subtask_count} alt göreve böldü`
        : 'Planner: doğrudan yanıt'
    });
    (tools || []).forEach(t => {
      const thought = TOOL_THOUGHTS[t] || TOOL_THOUGHTS.fallback;
      nodes.push({ kind: 'tool', icon: thought.icon, text: thought.text });
    });
    if (!tools || tools.length === 0) {
      nodes.push({ kind: 'executor', icon: '⚙️', text: 'Executor: tool gerekmedi' });
    }
    const verdict = agentTrace.critic_verdict || 'PASS';
    const criticIcon = verdict === 'PASS' ? '✅' : verdict === 'REVISE' ? '✏️' : '⏭️';
    const criticText = verdict === 'PASS'
      ? 'Critic: cevap doğrulandı'
      : verdict === 'REVISE'
      ? `Critic: ${agentTrace.critic_issues?.length || 0} sorun düzeltildi`
      : 'Critic: atlandı';
    nodes.push({ kind: 'critic', icon: criticIcon, text: criticText });
    return nodes;
  })();

  const nodeList = pipelineNodes || (tools || []).map(t => {
    const thought = TOOL_THOUGHTS[t] || TOOL_THOUGHTS.fallback;
    return { kind: 'tool', icon: thought.icon, text: thought.text };
  });

  const [visibleNodes, setVisibleNodes] = useState(0);

  useEffect(() => {
    if (nodeList.length === 0 || isError) return;
    setVisibleNodes(0);
    const interval = setInterval(() => {
      setVisibleNodes(prev => {
        if (prev >= nodeList.length) {
          clearInterval(interval);
          return prev;
        }
        return prev + 1;
      });
    }, 300);
    return () => clearInterval(interval);
  }, [nodeList.length, isError]);

  if (isError) {
    return React.createElement("div", { className: "thought-chain-wrap" },
      React.createElement("div", { className: "tc-node done", style: { animation: 'none', opacity: 1, transform: 'none', color: 'rgba(255, 71, 87, 0.8)', borderColor: 'rgba(255, 71, 87, 0.2)' } },
        React.createElement("span", { className: "tc-icon" }, '❌'),
        React.createElement("span", null, "AERA şu an yanıt veremedi. Tekrar deneyin.")
      )
    );
  }

  if (nodeList.length === 0) {
    return React.createElement("div", { className: "thought-chain-wrap" },
      React.createElement("div", { className: "tc-node done", style: { animation: 'none', opacity: 1, transform: 'none' } },
        React.createElement("span", { className: "tc-icon" }, '✓'),
        React.createElement("span", null, "AERA bu sefer ek araç kullanmadan yanıtladı.")
      )
    );
  }

  return React.createElement("div", { className: "thought-chain-wrap" },
    nodeList.slice(0, visibleNodes).map((node, idx) => {
      const isLastVisible = idx === visibleNodes - 1 && visibleNodes < nodeList.length;
      const isDone = idx < visibleNodes - 1 || visibleNodes === nodeList.length;
      return React.createElement(React.Fragment, { key: idx },
        React.createElement("div", { className: `tc-node tc-${node.kind} ${isLastVisible ? 'active' : ''} ${isDone ? 'done' : ''}` },
          React.createElement("span", { className: "tc-icon" }, node.icon),
          React.createElement("span", null, node.text)
        ),
        idx < nodeList.length - 1 && idx < visibleNodes && React.createElement("div", { className: "tc-line" })
      );
    })
  );
}
function ChatView({
  messages,
  loading,
  input,
  setInput,
  onSend,
  metrics,
  monthlyData,
  forecast,
  onUploadClick
}) {
  const chatRef = useRef(null);
  const chartRef = useRef(null);
  const chartInst = useRef(null);
  const forecastRef = useRef(null);
  const forecastInst = useRef(null);
  useEffect(() => {
    const el = chatRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, loading]);
  useEffect(() => {
    if (!monthlyData?.length || !chartRef.current) return;
    if (chartInst.current) {
      chartInst.current.destroy();
      chartInst.current = null;
    }
    chartInst.current = new Chart(chartRef.current, {
      type: 'bar',
      data: {
        labels: monthlyData.map(d => d.ay),
        datasets: [{
          label: 'Gelir',
          data: monthlyData.map(d => d.gelir),
          backgroundColor: 'rgba(0,230,140,.44)',
          borderColor: '#00E68C',
          borderWidth: 1,
          borderRadius: 5,
          borderSkipped: false
        }, {
          label: 'Gider',
          data: monthlyData.map(d => d.gider),
          backgroundColor: 'rgba(255,71,87,.44)',
          borderColor: '#FF4757',
          borderWidth: 1,
          borderRadius: 5,
          borderSkipped: false
        }]
      },
      options: {
        responsive: true,
        maintainAspectRatio: true,
        plugins: {
          legend: {
            labels: {
              color: 'rgba(255,255,255,.28)',
              font: {
                size: 11,
                family: 'var(--font)'
              }
            }
          },
          tooltip: {
            backgroundColor: 'rgba(7,8,13,.96)',
            borderColor: 'rgba(255,255,255,.04)',
            borderWidth: 1,
            callbacks: {
              label: ctx => ` ${ctx.dataset.label}: ${ctx.parsed.y.toLocaleString('tr-TR')} ₺`
            }
          }
        },
        scales: {
          x: {
            ticks: {
              color: 'rgba(255,255,255,.22)',
              font: {
                size: 10
              }
            },
            grid: {
              color: 'rgba(255,255,255,.022)'
            }
          },
          y: {
            ticks: {
              color: 'rgba(255,255,255,.22)',
              font: {
                size: 10
              },
              callback: v => (v / 1000).toFixed(0) + 'K ₺'
            },
            grid: {
              color: 'rgba(255,255,255,.022)'
            }
          }
        }
      }
    });
    return () => {
      if (chartInst.current) {
        chartInst.current.destroy();
        chartInst.current = null;
      }
    };
  }, [monthlyData]);
  useEffect(() => {
    if (!forecast?.projeksiyon?.length || !forecastRef.current) return;
    if (forecastInst.current) { forecastInst.current.destroy(); forecastInst.current = null; }
    const proj = forecast.projeksiyon;
    forecastInst.current = new Chart(forecastRef.current, {
      type: 'line',
      data: {
        labels: proj.map(d => d.ay),
        datasets: [
          { label: 'CI Üst', data: proj.map(d => d.ci_ust), fill: '+1', borderWidth: 0, borderColor: 'transparent', backgroundColor: 'rgba(255,215,0,.13)', pointRadius: 0, tension: 0.4 },
          { label: 'CI Alt', data: proj.map(d => d.ci_alt), fill: false, borderWidth: 0, borderColor: 'transparent', backgroundColor: 'transparent', pointRadius: 0, tension: 0.4 },
          { label: 'Tahmini Net', data: proj.map(d => d.tahmini_net), borderColor: '#FFD700', borderWidth: 2, fill: false, tension: 0.4, pointRadius: 4, backgroundColor: '#FFD700' },
          { label: 'Tahmini Gelir', data: proj.map(d => d.tahmini_gelir), borderColor: '#00E68C', borderWidth: 1.5, borderDash: [4, 3], fill: false, tension: 0.4, pointRadius: 3, backgroundColor: '#00E68C' },
          { label: 'Tahmini Gider', data: proj.map(d => d.tahmini_gider), borderColor: '#FF4757', borderWidth: 1.5, borderDash: [4, 3], fill: false, tension: 0.4, pointRadius: 3, backgroundColor: '#FF4757' }
        ]
      },
      options: {
        responsive: true, maintainAspectRatio: true,
        plugins: {
          legend: { labels: { color: 'rgba(255,255,255,.28)', font: { size: 10, family: 'var(--font)' }, filter: item => item.datasetIndex >= 2 } },
          tooltip: { backgroundColor: 'rgba(7,8,13,.96)', borderColor: 'rgba(255,255,255,.04)', borderWidth: 1, filter: item => item.datasetIndex >= 2, callbacks: { label: ctx => ` ${ctx.dataset.label}: ${ctx.parsed.y.toLocaleString('tr-TR')} ₺` } }
        },
        scales: {
          x: { ticks: { color: 'rgba(255,255,255,.22)', font: { size: 10 } }, grid: { color: 'rgba(255,255,255,.022)' } },
          y: { ticks: { color: 'rgba(255,255,255,.22)', font: { size: 10 }, callback: v => (v / 1000).toFixed(0) + 'K ₺' }, grid: { color: 'rgba(255,255,255,.022)' } }
        }
      }
    });
    return () => { if (forecastInst.current) { forecastInst.current.destroy(); forecastInst.current = null; } };
  }, [forecast]);
  function onKey(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      onSend(input);
    }
  }
  return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(MetricsRow, {
    metrics: metrics
  }), monthlyData?.length > 0 && /*#__PURE__*/React.createElement("div", {
    className: "chart-sec"
  }, /*#__PURE__*/React.createElement("div", {
    className: "chart-wrap"
  }, /*#__PURE__*/React.createElement("div", {
    className: "chart-title"
  }, "AYLIK GEL\u0130R / G\u0130DER"), /*#__PURE__*/React.createElement("canvas", {
    ref: chartRef,
    style: {
      maxHeight: 130
    }
  }))), forecast?.projeksiyon?.length > 0 && /*#__PURE__*/React.createElement("div", {
    className: "chart-sec"
  }, /*#__PURE__*/React.createElement("div", {
    className: "chart-wrap"
  }, /*#__PURE__*/React.createElement("div", {
    className: "chart-title",
    style: { display: 'flex', alignItems: 'center', gap: 8 }
  }, /*#__PURE__*/React.createElement("span", null, "NAK\u0130T AKI\u015e PROJEKS\u0130YONU"), /*#__PURE__*/React.createElement("span", {
    style: { fontStyle: 'italic', textTransform: 'none', letterSpacing: 'normal', color: 'rgba(255,215,0,.7)', fontSize: 8 }
  }, forecast.tahmin_yontemi || '')), /*#__PURE__*/React.createElement("canvas", {
    ref: forecastRef,
    style: { maxHeight: 130 }
  }), /*#__PURE__*/React.createElement("div", {
    style: { display: 'flex', gap: 14, marginTop: 7, fontSize: 9.5, color: 'rgba(255,255,255,.38)', flexWrap: 'wrap' }
  }, /*#__PURE__*/React.createElement("span", null,
    (forecast.aylik_trend_gelir >= 0 ? '↗ Gelir trendi: +' : '↘ Gelir trendi: ') +
    (forecast.aylik_trend_gelir != null ? Math.abs(forecast.aylik_trend_gelir).toLocaleString('tr-TR') + ' ₺/ay' : '—')
  ), /*#__PURE__*/React.createElement("span", null,
    (forecast.aylik_trend_gider >= 0 ? '↗ Gider trendi: +' : '↘ Gider trendi: ') +
    (forecast.aylik_trend_gider != null ? Math.abs(forecast.aylik_trend_gider).toLocaleString('tr-TR') + ' ₺/ay' : '—')
  ), /*#__PURE__*/React.createElement("span", { style: { marginLeft: 'auto', color: 'rgba(255,215,0,.45)' } }, '%90 G\xfcven Aralığı'))
  )), /*#__PURE__*/React.createElement("div", {
    className: "chat-area",
    ref: chatRef
  }, messages.length === 0 ? /*#__PURE__*/React.createElement("div", { className: "dash-empty" }, 
    /*#__PURE__*/React.createElement(BreathingCore, { status: loading ? 'thinking' : 'idle', hasError: false }),
    !loading && /*#__PURE__*/React.createElement("div", {
      className: "wl-grid", style: { marginTop: 20, width: '100%', maxWidth: 840, transform: 'scale(1.05)' }
    }, SUGGESTIONS.map((s, i) => /*#__PURE__*/React.createElement("button", {
      key: i,
      className: "wl-card",
      onClick: () => onSend(s.cmd)
    }, /*#__PURE__*/React.createElement("span", {
      className: "wl-arrow"
    }, s.icon), /*#__PURE__*/React.createElement("span", {
      className: "wl-body"
    }, /*#__PURE__*/React.createElement("span", {
      className: "wl-lbl"
    }, s.label), /*#__PURE__*/React.createElement("span", {
      className: "wl-sub"
    }, s.desc)))))
  ) : messages.map(m => /*#__PURE__*/React.createElement("div", {
    key: m.id,
    className: `msg ${m.role}`
  }, m.role === 'bot' ? /*#__PURE__*/React.createElement(React.Fragment, null,
    (m.tools?.length > 0 || m.agentTrace) && /*#__PURE__*/React.createElement(ThoughtChain, { tools: m.tools, isError: m.isError, agentTrace: m.agentTrace }),
    /*#__PURE__*/React.createElement("div", {
      className: "msg-bbl",
      dangerouslySetInnerHTML: {
        __html: renderMarkdown(m.text)
      }
    })
  ) : /*#__PURE__*/React.createElement("div", {
    className: "msg-bbl",
    style: {
      whiteSpace: 'pre-wrap'
    }
  }, m.text), m.role === 'bot' && m.latency && /*#__PURE__*/React.createElement("div", {
    className: "msg-meta"
  }, /*#__PURE__*/React.createElement("span", {
    className: "msg-lat"
  }, "\u26A1 ", m.latency, "ms")))), loading && /*#__PURE__*/React.createElement("div", {
    className: "msg bot"
  }, /*#__PURE__*/React.createElement("div", {
    className: "thinking"
  }, /*#__PURE__*/React.createElement("span", {
    className: "tk"
  }), /*#__PURE__*/React.createElement("span", {
    className: "tk"
  }), /*#__PURE__*/React.createElement("span", {
    className: "tk"
  }), /*#__PURE__*/React.createElement("span", {
    className: "tk-lbl"
  }, "Otonom analiz \xE7al\u0131\u015F\u0131yor...")))), !loading && messages.length > 0 && /*#__PURE__*/React.createElement("div", {
    className: "quick-bar"
  }, ['📊 Likidite analizi', '🏥 Sağlık skoru', '📈 Nakit projeksiyonu', '🏛️ Fon taraması', '💸 Gider optimizasyonu', '⚠️ Risk raporu', '🔍 Anomali tespiti', '📉 Nakit sıkışıklığı'].map(s => /*#__PURE__*/React.createElement("button", {
    key: s,
    className: "quick-chip",
    onClick: () => onSend(s)
  }, s))), /*#__PURE__*/React.createElement("div", {
    className: "inp-area"
  }, /*#__PURE__*/React.createElement("div", {
    className: "inp-wrap"
  }, /*#__PURE__*/React.createElement("button", {
    className: "upload-btn",
    onClick: onUploadClick,
    title: "Veri Yükle (CSV / Excel)",
    style: { display: 'flex', alignItems: 'center', justifyContent: 'center', width: 34, height: 34, borderRadius: 17, border: 'none', background: 'transparent', color: 'var(--muted)', cursor: 'pointer', transition: 'all 0.2s ease', alignSelf: 'flex-end', marginBottom: 4 }
  }, /*#__PURE__*/React.createElement("svg", {
    width: "20",
    height: "20",
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: "2",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  }, /*#__PURE__*/React.createElement("line", { x1: "12", y1: "5", x2: "12", y2: "19" }), /*#__PURE__*/React.createElement("line", { x1: "5", y1: "12", x2: "19", y2: "12" }))), /*#__PURE__*/React.createElement("textarea", {
    className: "inp",
    value: input,
    onChange: e => {
      setInput(e.target.value);
      e.target.style.height = 'auto';
      e.target.style.height = Math.min(e.target.scrollHeight, 120) + 'px';
    },
    onKeyDown: onKey,
    rows: 1,
    placeholder: "Finansal durumunuzu sorun..."
  }), /*#__PURE__*/React.createElement("button", {
    className: "send-btn",
    onClick: () => onSend(input),
    disabled: loading || !input.trim()
  }, /*#__PURE__*/React.createElement("svg", {
    width: "18",
    height: "18",
    viewBox: "0 0 24 24",
    fill: "currentColor"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"
  }))))));
}

// WHAT-IF VIEW
function WhatIfView({
  sessionId,
  addMsg,
  setMainView
}) {
  const [income, setIncome] = useState(0);
  const [expense, setExpense] = useState(0);
  const [months, setMonths] = useState(3);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState(null);
  async function runSim() {
    if (!sessionId) {
      setResult({
        error: 'Simülasyon için önce demo veya CSV veri yükleyin.'
      });
      return;
    }
    setRunning(true);
    setResult(null);
    const cmd = `simulate_scenario aracını kullanarak what-if simülasyonu yap: extra_monthly_income=${income}, extra_monthly_expense=${expense}, months=${months}. Türkçe detaylı sonuç ver.`;
    try {
      const res = await fetch(`${API}/api/chat`, {
        method: 'POST',
        headers: getAuthHeaders(),
        body: JSON.stringify({
          message: cmd,
          session_id: sessionId
        })
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.detail || data.error?.message || 'Hata');
      setResult({
        text: stripBlocks(data.reply),
        tools: data.tools_used,
        latency: data.latency_ms,
        agentTrace: data.agent_trace
      });
    } catch (e) {
      setResult({
        error: e.message
      });
    } finally {
      setRunning(false);
    }
  }
  const netEffect = income - expense;
  return /*#__PURE__*/React.createElement("div", {
    className: "wi-view"
  }, /*#__PURE__*/React.createElement("div", {
    className: "wi-header"
  }, /*#__PURE__*/React.createElement("div", {
    className: "wi-title"
  }, "Senaryo Sim\xFClat\xF6r\xFC"), /*#__PURE__*/React.createElement("div", {
    className: "wi-sub"
  }, "Finansal parametreleri de\u011Fi\u015Ftirerek gelecek nakit ak\u0131\u015F\u0131n\u0131 modelleyin \u2014 What-If analizi, kira art\u0131\u015F\u0131, i\u015Fe al\u0131m, fiyat de\u011Fi\u015Fikli\u011Fi senaryolar\u0131")), /*#__PURE__*/React.createElement("div", {
    className: "wi-grid"
  }, /*#__PURE__*/React.createElement("div", {
    className: "wi-card"
  }, /*#__PURE__*/React.createElement("div", {
    className: "wi-card-title"
  }, "\uD83D\uDCC8 Ayl\u0131k Gelir De\u011Fi\u015Fimi"), /*#__PURE__*/React.createElement("input", {
    type: "range",
    className: "wi-slider",
    min: -500000,
    max: 500000,
    step: 5000,
    value: income,
    onChange: e => setIncome(Number(e.target.value))
  }), /*#__PURE__*/React.createElement("div", {
    className: "wi-val",
    style: {
      color: income >= 0 ? 'var(--green)' : 'var(--red)'
    }
  }, income >= 0 ? '+' : '', fmtTL(income), " \u20BA / ay"), /*#__PURE__*/React.createElement("div", {
    className: "wi-hint"
  }, income === 0 ? 'Değişiklik yok' : income > 0 ? 'Ek gelir senaryosu (yeni müşteri, fiyat artışı)' : 'Gelir düşüş senaryosu (müşteri kaybı, sezon)')), /*#__PURE__*/React.createElement("div", {
    className: "wi-card"
  }, /*#__PURE__*/React.createElement("div", {
    className: "wi-card-title"
  }, "\uD83D\uDCC9 Ayl\u0131k Gider De\u011Fi\u015Fimi"), /*#__PURE__*/React.createElement("input", {
    type: "range",
    className: "wi-slider",
    min: -300000,
    max: 300000,
    step: 5000,
    value: expense,
    onChange: e => setExpense(Number(e.target.value))
  }), /*#__PURE__*/React.createElement("div", {
    className: "wi-val",
    style: {
      color: expense <= 0 ? 'var(--green)' : 'var(--orange)'
    }
  }, expense >= 0 ? '+' : '', fmtTL(expense), " \u20BA / ay"), /*#__PURE__*/React.createElement("div", {
    className: "wi-hint"
  }, expense === 0 ? 'Değişiklik yok' : expense > 0 ? 'Ek gider senaryosu (işe alım, kira artışı)' : 'Tasarruf senaryosu (optimizasyon, vazgeçme)')), /*#__PURE__*/React.createElement("div", {
    className: "wi-card"
  }, /*#__PURE__*/React.createElement("div", {
    className: "wi-card-title"
  }, "\uD83D\uDCC5 Projeksiyon Periyodu"), /*#__PURE__*/React.createElement("div", {
    className: "wi-months"
  }, [1, 2, 3, 4, 5, 6].map(m => /*#__PURE__*/React.createElement("button", {
    key: m,
    className: `wi-month${months === m ? ' active' : ''}`,
    onClick: () => setMonths(m)
  }, m, " ay"))), /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: 8,
      padding: '8px 10px',
      background: 'var(--surf2)',
      borderRadius: 8
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 9,
      color: 'var(--muted)',
      textTransform: 'uppercase',
      letterSpacing: '.06em',
      marginBottom: 4
    }
  }, "NET ETK\u0130 / AY"), /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 18,
      fontWeight: 700,
      fontFamily: 'var(--mono)',
      color: netEffect >= 0 ? 'var(--green)' : 'var(--red)'
    }
  }, netEffect >= 0 ? '+' : '', fmtTL(netEffect), " \u20BA")), /*#__PURE__*/React.createElement("div", {
    className: "wi-hint",
    style: {
      marginTop: 6
    }
  }, months, " ayl\u0131k otonom projeksiyon"))), (income !== 0 || expense !== 0) && /*#__PURE__*/React.createElement("div", {
    className: "wi-preview"
  }, /*#__PURE__*/React.createElement("div", {
    className: "wi-preview-title"
  }, "Senaryo Parametreleri"), /*#__PURE__*/React.createElement("div", {
    className: "wi-preview-items"
  }, income !== 0 && /*#__PURE__*/React.createElement("div", {
    className: "wi-preview-item",
    style: {
      color: income > 0 ? 'var(--green)' : 'var(--red)'
    }
  }, income > 0 ? '↑' : '↓', " Gelir: ", income >= 0 ? '+' : '', fmtTL(income), " \u20BA/ay"), expense !== 0 && /*#__PURE__*/React.createElement("div", {
    className: "wi-preview-item",
    style: {
      color: expense > 0 ? 'var(--orange)' : 'var(--green)'
    }
  }, expense > 0 ? '↑' : '↓', " Gider: ", expense >= 0 ? '+' : '', fmtTL(expense), " \u20BA/ay"), /*#__PURE__*/React.createElement("div", {
    className: "wi-preview-item",
    style: {
      color: 'var(--acc)'
    }
  }, "\u2261 K\xFCm\xFClatif ", months, " ayl\u0131k net: ", fmtTL(netEffect * months), " \u20BA"))), /*#__PURE__*/React.createElement("button", {
    className: "wi-run",
    onClick: runSim,
    disabled: running || !sessionId
  }, running ? /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("span", {
    className: "tk"
  }), /*#__PURE__*/React.createElement("span", {
    className: "tk"
  }), /*#__PURE__*/React.createElement("span", {
    className: "tk"
  }), " Sim\xFCle ediliyor...") : '▶ Simülasyonu Çalıştır'), !sessionId && /*#__PURE__*/React.createElement("div", {
    className: "wi-no-data"
  }, "\u26A0 \xD6nce sol panelden bir demo se\xE7in veya CSV y\xFCkleyin.", /*#__PURE__*/React.createElement("button", {
    className: "wi-go",
    onClick: () => setMainView('chat')
  }, "\u2192 Veri Y\xFCkle")), result && /*#__PURE__*/React.createElement("div", {
    className: "wi-result"
  }, result.error ? /*#__PURE__*/React.createElement("div", {
    style: {
      color: 'var(--red)'
    }
  }, result.error) : /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("div", {
    className: "wi-result-title"
  }, "Sim\xFClasyon Sonucu", result.latency && /*#__PURE__*/React.createElement("span", {
    style: {
      fontSize: 10,
      opacity: .45,
      marginLeft: 8
    }
  }, "\u26A1 ", result.latency, "ms")), /*#__PURE__*/React.createElement("div", {
    className: "wi-result-body",
    dangerouslySetInnerHTML: {
      __html: renderMarkdown(result.text)
    }
  }), result.tools?.length > 0 && /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: 10,
      display: 'flex',
      gap: 4,
      flexWrap: 'wrap'
    }
  }, result.tools.map((t, i) => /*#__PURE__*/React.createElement("span", {
    key: i,
    className: "tool-pill"
  }, "\u2699 ", toolLabel(t)))))));
}

// DASHBOARD VIEW
function DashboardView({
  metrics,
  monthlyData,
  onDemo,
  setView
}) {
  if (!metrics) return /*#__PURE__*/React.createElement("div", {
    className: "dash-empty"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      marginBottom: 14,
      color: 'var(--acc)',
      opacity: 0.8
    }
  }, IcChartAlt(52)), /*#__PURE__*/React.createElement("div", {
    className: "dash-empty-title"
  }, "Dashboard i\xE7in Veri Gerekli"), /*#__PURE__*/React.createElement("div", {
    style: {
      color: 'var(--muted)',
      fontSize: 12.5,
      marginBottom: 22
    }
  }, "Sol panelden demo se\xE7in veya CSV y\xFCkleyin."), /*#__PURE__*/React.createElement("button", {
    className: "sb-demo",
    style: {
      display: 'inline-flex',
      width: 'auto',
      padding: '10px 20px'
    },
    onClick: () => onDemo(null)
  }, IcFlask(16), " ", /*#__PURE__*/React.createElement("span", null, "Demo Veri Seti")));
  return /*#__PURE__*/React.createElement("div", {
    className: "dash-view"
  }, metrics.healthSkor != null && /*#__PURE__*/React.createElement("div", {
    className: "dash-health"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      textAlign: 'center'
    }
  }, /*#__PURE__*/React.createElement("div", {
    className: "dash-h-label"
  }, "F\u0130NANSAL SA\u011eLIK"), /*#__PURE__*/React.createElement("div", {
    className: "dash-h-score",
    style: {
      color: scoreColor(metrics.healthSkor)
    }
  }, metrics.healthSkor), /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 11,
      color: 'var(--muted)'
    }
  }, "/100")), /*#__PURE__*/React.createElement("div", {
    className: "dash-divider"
  }), /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 34,
      lineHeight: 1,
      marginBottom: 7
    }
  }, metrics.healthEmoji, " ", /*#__PURE__*/React.createElement("span", {
    style: {
      fontWeight: 700,
      fontSize: 26,
      color: scoreColor(metrics.healthSkor)
    }
  }, metrics.healthHarf)), /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 15,
      fontWeight: 600,
      color: scoreColor(metrics.healthSkor)
    }
  }, metrics.healthSkor >= 80 ? 'Finansal Açıdan Sağlıklı' : metrics.healthSkor >= 60 ? 'Dikkat Gerektiriyor' : metrics.healthSkor >= 40 ? 'Yüksek Risk' : 'Kritik Durum'), /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 10,
      color: 'var(--muted)',
      marginTop: 8,
      display: 'flex',
      flexDirection: 'column',
      gap: 3
    }
  }, /*#__PURE__*/React.createElement("span", { style: { color: 'var(--acc)' } }, "\u25CF Gemini 2.5 Flash Motoru"), /*#__PURE__*/React.createElement("span", { style: { color: metrics.risk === 'KR\u0130T\u0130K' || metrics.risk === 'Y\u00dcKSEK' ? 'var(--orange)' : 'var(--green)' } }, "\u25CF Risk: ", metrics.risk || 'Hesaplanıyor'), /*#__PURE__*/React.createElement("span", null, "\u25CF Nakit \u00d6mr\u00fc: ", metrics.runway === '\u221E' ? 'Pozitif Akış' : (metrics.runway || '—') + ' ay'))), /*#__PURE__*/React.createElement("div", {
    style: {
      marginLeft: 'auto',
      display: 'flex',
      gap: 24,
      flexWrap: 'wrap'
    }
  }, [{
    l: 'Net Nakit',
    v: fmtTL(metrics.net) + ' ₺',
    c: 'var(--acc)'
  }, {
    l: 'Nakit Yakımı',
    v: fmtTL(metrics.burn) + ' ₺/ay',
    c: 'var(--orange)'
  }, {
    l: 'Nakit Ömrü',
    v: metrics.runway === '∞' ? '∞ ay' : metrics.runway + ' ay',
    c: metrics.runway === '∞' ? 'var(--green)' : parseFloat(metrics.runway) < 3 ? 'var(--red)' : 'var(--green)'
  }, {
    l: 'Risk',
    v: riskEmoji(metrics.risk),
    c: 'var(--text)'
  }].map(k => /*#__PURE__*/React.createElement("div", {
    key: k.l,
    style: {
      textAlign: 'center'
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 9,
      color: 'var(--muted)',
      textTransform: 'uppercase',
      letterSpacing: '.08em',
      marginBottom: 4
    }
  }, k.l), /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 15,
      fontWeight: 700,
      color: k.c,
      fontFamily: 'var(--mono)'
    }
  }, k.v))))), monthlyData.length > 0 && /*#__PURE__*/React.createElement("div", {
    className: "dash-monthly"
  }, /*#__PURE__*/React.createElement("div", {
    className: "dash-section-title"
  }, "AYLIK LİKİDİTE & NAKİT YAKIMI"), (() => {
    const max = Math.max(...monthlyData.map(d => Math.max(d.gelir || 0, d.gider || 0)), 1);
    return monthlyData.map((d, i) => /*#__PURE__*/React.createElement("div", {
      key: i,
      className: "dash-month-row"
    }, /*#__PURE__*/React.createElement("div", {
      className: "dash-month-label"
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        fontWeight: 600,
        color: 'var(--text)',
        minWidth: 55,
        fontFamily: 'var(--mono)',
        fontSize: 10
      }
    }, d.ay), /*#__PURE__*/React.createElement("span", {
      style: {
        color: 'var(--green)'
      }
    }, fmtTL(d.gelir), " \u20BA"), /*#__PURE__*/React.createElement("span", {
      style: {
        color: 'var(--red)'
      }
    }, fmtTL(d.gider), " \u20BA"), /*#__PURE__*/React.createElement("span", {
      style: {
        color: d.gelir - d.gider >= 0 ? 'var(--acc)' : 'var(--red)',
        fontWeight: 600
      }
    }, d.gelir - d.gider >= 0 ? '+' : '', fmtTL(d.gelir - d.gider), " \u20BA")), /*#__PURE__*/React.createElement("div", {
      className: "dash-bar-wrap"
    }, /*#__PURE__*/React.createElement("div", {
      className: "dash-bar",
      style: {
        background: 'var(--green)',
        width: (d.gelir || 0) / max * 100 + '%'
      }
    })), /*#__PURE__*/React.createElement("div", {
      className: "dash-bar-wrap"
    }, /*#__PURE__*/React.createElement("div", {
      className: "dash-bar",
      style: {
        background: 'var(--red)',
        width: (d.gider || 0) / max * 100 + '%'
      }
    }))));
  })()), /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      gap: 8,
      flexWrap: 'wrap'
    }
  }, [['💬 Detaylı Analiz', 'chat'], ['📈 Projeksiyon', 'chat'], ['⚗️ What-If', 'whatif'], ['🏛️ Teşvik Haritası', 'incentives']].map(([l, v]) => /*#__PURE__*/React.createElement("button", {
    key: l,
    className: "sb-demo",
    style: {
      width: 'auto',
      padding: '7px 14px',
      fontSize: 11
    },
    onClick: () => setView(v)
  }, l))));
}

// INCENTIVES VIEW
function IncentivesView({
  incentives,
  setView,
  onSend
}) {
  if (incentives.length === 0) return /*#__PURE__*/React.createElement("div", {
    className: "dash-empty"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      marginBottom: 14,
      color: 'var(--acc)',
      opacity: 0.8
    }
  }, IcBank(52)), /*#__PURE__*/React.createElement("div", {
    className: "dash-empty-title"
  }, "Te\u015Fvik & Arbitraj Radar\u0131"), /*#__PURE__*/React.createElement("div", {
    style: {
      color: 'var(--muted)',
      fontSize: 12.5,
      marginBottom: 22
    }
  }, "AeraCFO, firman\u0131z\u0131n profiline uygun KOSGEB, T\xDCB\u0130TAK ve Bakanl\u0131k desteklerini otomatik tarar."), /*#__PURE__*/React.createElement("button", {
    className: "sb-demo",
    style: {
      display: 'inline-flex',
      width: 'auto',
      padding: '10px 20px'
    },
    onClick: () => {
      setView('chat');
      setTimeout(() => onSend('Firmam için uygun devlet hibe ve fon programlarını tara'), 200);
    }
  }, "\uD83D\uDD0D Te\u015Fvik Taramas\u0131 Ba\u015Flat"));
  return /*#__PURE__*/React.createElement("div", {
    className: "inc-view"
  }, /*#__PURE__*/React.createElement("div", {
    className: "inc-header"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 11,
      color: 'var(--muted)',
      fontWeight: 600,
      textTransform: 'uppercase',
      letterSpacing: '.06em'
    }
  }, incentives.length, " Te\u015Fvik Program\u0131 \u2014 AeraCFO Te\u015Fvik Veritaban\u0131 v2.0")), /*#__PURE__*/React.createElement("div", {
    className: "inc-grid"
  }, incentives.map((inc, i) => /*#__PURE__*/React.createElement("div", {
    key: i,
    className: "inc-card",
    onMouseOver: e => e.currentTarget.style.borderColor = 'rgba(255,215,0,.38)',
    onMouseOut: e => e.currentTarget.style.borderColor = 'rgba(255,215,0,.15)'
  }, /*#__PURE__*/React.createElement("div", {
    className: "inc-card-type"
  }, "Hibe Program\u0131"), /*#__PURE__*/React.createElement("div", {
    className: "inc-card-name"
  }, inc.isim), /*#__PURE__*/React.createElement("div", {
    className: "inc-card-amt"
  }, inc.tutar), /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      gap: 5,
      flexWrap: 'wrap',
      marginBottom: 10
    }
  }, /*#__PURE__*/React.createElement("span", {
    className: "sb-inc-badge"
  }, inc.tip), (inc.etiketler || []).slice(0, 3).map(e => /*#__PURE__*/React.createElement("span", {
    key: e,
    style: {
      background: 'var(--surf2)',
      color: 'var(--muted)',
      border: '1px solid var(--border)',
      padding: '1px 7px',
      borderRadius: 10,
      fontSize: 9
    }
  }, e))), /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 11,
      color: 'var(--muted)',
      borderTop: '1px solid var(--border)',
      paddingTop: 8
    }
  }, "\uD83D\uDD17 ", inc.basvuru)))), /*#__PURE__*/React.createElement("button", {
    className: "sb-demo",
    style: {
      width: 'auto',
      display: 'inline-flex',
      padding: '8px 16px',
      fontSize: 11,
      marginTop: 4
    },
    onClick: () => {
      setView('chat');
      setTimeout(() => onSend('Bu teşvikler için başvuru şartlarını ve süreçlerini detaylı açıkla'), 200);
    }
  }, "\uD83D\uDCAC Ba\u015Fvuru Detaylar\u0131n\u0131 Sorgula"));
}

// SETTINGS PANEL
function SettingsPanel({
  accent,
  setAccent,
  bgAnim,
  setBgAnim,
  theme,
  setTheme,
  apiKey,
  setApiKey,
  onClose
}) {
  const opts = [['cyan', '#00D4E5'], ['green', '#00E68C'], ['purple', '#8B5CF6'], ['amber', '#F59E0B']];
  return /*#__PURE__*/React.createElement("div", {
    className: "settings-overlay",
    onClick: onClose
  }, /*#__PURE__*/React.createElement("div", {
    className: "settings-panel",
    onClick: e => e.stopPropagation()
  }, /*#__PURE__*/React.createElement("div", {
    className: "settings-hdr"
  }, /*#__PURE__*/React.createElement("span", null, "Sistem Ayarlar\u0131"), /*#__PURE__*/React.createElement("button", {
    onClick: onClose,
    style: {
      background: 'none',
      border: 'none',
      color: 'var(--muted)',
      cursor: 'pointer',
      fontSize: 16,
      padding: '4px',
      lineHeight: 1
    }
  }, "\u2715")), /*#__PURE__*/React.createElement("div", {
    className: "settings-body"
  }, /*#__PURE__*/React.createElement("div", {
    className: "settings-row"
  }, /*#__PURE__*/React.createElement("span", null, "Vurgu Rengi"), /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      gap: 6
    }
  }, opts.map(([id, clr]) => /*#__PURE__*/React.createElement("button", {
    key: id,
    onClick: () => setAccent(id),
    style: {
      width: 24,
      height: 24,
      borderRadius: 6,
      background: clr,
      border: `2.5px solid ${accent === id ? 'white' : 'transparent'}`,
      cursor: 'pointer',
      transition: '.14s'
    }
  })))), /*#__PURE__*/React.createElement("div", {
    className: "settings-row"
  }, /*#__PURE__*/React.createElement("span", null, "Arka Plan Animasyonu"), /*#__PURE__*/React.createElement("button", {
    className: `tog${bgAnim ? ' on' : ''}`,
    onClick: () => setBgAnim(!bgAnim)
  }, /*#__PURE__*/React.createElement("i", null))), /*#__PURE__*/React.createElement("div", {
    className: "settings-row",
    style: {
      flexDirection: 'column',
      alignItems: 'flex-start',
      gap: 8,
      borderTop: '1px solid var(--border)',
      paddingTop: 12,
      marginTop: 6
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontSize: 11,
      fontWeight: 600,
      color: 'var(--text)'
    }
  }, "Özel Gemini API Anahtarı"), /*#__PURE__*/React.createElement("input", {
    type: "password",
    placeholder: "AIzaSy...",
    value: apiKey,
    onChange: e => setApiKey(e.target.value),
    style: {
      width: '100%',
      padding: '8px 12px',
      borderRadius: 6,
      border: '1px solid var(--border)',
      background: 'rgba(255,255,255,0.02)',
      color: 'var(--text)',
      fontFamily: 'var(--mono)',
      fontSize: 11,
      marginTop: 4
    }
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 10,
      color: 'var(--muted)',
      lineHeight: 1.55,
      borderTop: '1px solid var(--border)',
      paddingTop: 12
    }
  }, "AeraCFO v2 \xB7 Otonom KOB\u0130 CFO Sistemi", /*#__PURE__*/React.createElement("br", null), "Rust/Axum \xB7 Polars \xB7 Gemini 2.5 Flash \xB7 10 Ara\xE7"))));
}

// MAIN APP
function App() {
  const [appView, setAppView] = useState('landing');
  const [chatView, setChatView] = useState('chat');
  const [messages, setMessages] = useState([]);
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [sessionId, setSessionId] = useState(null);
  const [upStatus, setUpStatus] = useState(null);
  const [metrics, setMetrics] = useState(null);
  const [monthly, setMonthly] = useState([]);
  const [forecast, setForecast] = useState(null);
  const [incentives, setIncentives] = useState([]);
  const [serverOk, setServerOk] = useState(false);
  const [accent, setAccent] = useState('cyan');
  const [bgAnim, setBgAnim] = useState(true);
  const [theme, setTheme] = useState('dark');
  const [apiKey, setApiKey] = useState(() => localStorage.getItem('aera_api_key') || '');
  const [showSett, setShowSett] = useState(false);
  const [showPdf, setShowPdf] = useState(false);
  const [lastAnalysis, setLastAnalysis] = useState(null);
  const [scenarioName, setScenarioName] = useState(null);
  const fileRef = useRef(null);
  const HUES = {
    cyan: '#00D4E5',
    green: '#00E68C',
    purple: '#8B5CF6',
    amber: '#F59E0B'
  };
  useEffect(() => {
    const r = document.documentElement;
    const clr = HUES[accent] || HUES.cyan;
    r.style.setProperty('--acc', clr);
    r.style.setProperty('--acc-dim', clr + '1A');
    document.body.classList.toggle('no-bg-anim', !bgAnim);
  }, [accent, bgAnim]);
  useEffect(() => {
    document.body.classList.remove('light-theme');
  }, [theme]);
  useEffect(() => {
    localStorage.setItem('aera_api_key', apiKey);
  }, [apiKey]);
  useEffect(() => {
    fetch(`${API}/health`).then(r => r.json()).then(d => setServerOk(d.status === 'operational')).catch(() => {});
  }, []);
  useEffect(() => {
    const root = document.documentElement;
    if (metrics?.healthSkor != null) {
      const s = metrics.healthSkor;
      if (s >= 80) root.style.setProperty('--ambient-color', 'rgba(0, 230, 140, 0.08)');
      else if (s >= 60) root.style.setProperty('--ambient-color', 'rgba(255, 211, 42, 0.08)');
      else if (s >= 40) root.style.setProperty('--ambient-color', 'rgba(255, 160, 64, 0.08)');
      else root.style.setProperty('--ambient-color', 'rgba(255, 71, 87, 0.08)');
    } else {
      root.style.setProperty('--ambient-color', 'rgba(0, 212, 229, 0.03)');
    }
  }, [metrics?.healthSkor]);
  const addMsg = (role, text, meta = {}) => setMessages(prev => [...prev, {
    role,
    text,
    ...meta,
    id: Date.now() + Math.random()
  }]);
  function handleUpload(data, name, sid) {
    setForecast(null);
    setIncentives([]);
    setMessages([]);
    if (data.monthly_data?.length) {
      setMonthly(data.monthly_data);
      // Hızlı yükleme: Veriyi hemen metrik hesaplamasında kullan
      const md = data.monthly_data;
      const totalGelir = md.reduce((s, m) => s + (m.gelir || 0), 0);
      const totalGider = md.reduce((s, m) => s + (m.gider || 0), 0);
      const net = totalGelir - totalGider;
      const avgGider = totalGider / md.length;
      // Cash reserve ay cinsi — backend'in cash_reserve_months formülüyle aynı.
      // Eski formül "|net|/(avgGider-avgGelir)" cebirsel olarak N'e sadeleşip
      // her durumda veri ay sayısını veriyor, üstüne gelir>gider olunca 999 atayıp
      // teknoloji_startup gibi zarardaki şirketleri "DÜŞÜK" gösteriyordu.
      const reserveMonths = avgGider > 0 ? net / avgGider : 0;
      const risk = reserveMonths < 1 ? 'KRİTİK'
        : reserveMonths < 3 ? 'YÜKSEK'
        : reserveMonths < 6 ? 'ORTA' : 'DÜŞÜK';
      setMetrics({
        net: net.toFixed(2),
        risk,
        burn: avgGider.toFixed(2),
        // Negatif rezerv → "kaybediyor". UI '∞' yerine işaretli ay göster.
        runway: reserveMonths >= 999 ? '∞' : reserveMonths.toFixed(1),
        healthSkor: null,
        healthHarf: null,
        healthEmoji: null,
      });
    } else {
      setMetrics(null);
    }
    const dr = data.date_range;
    addMsg('bot', `📊 **${name || 'Veri'}** yüklendi!\n• ${data.rows} kayıt, ${data.columns} sütun\n` + `• Sütunlar: ${data.column_names.join(', ')}\n` + (dr ? `• Tarih: ${dr.start} → ${dr.end} (${dr.days} gün)\n` : '') + `\n⏳ Proaktif finansal analiz başlatılıyor...`);
    const isFile = name && (name.toLowerCase().endsWith('.csv') || name.toLowerCase().endsWith('.xlsx'));
    const ctx = name && name !== 'Veri' && !isFile ? ` Sektör: ${name}.` : '';
    setTimeout(() => {
      send(`Verimi yükledim.${ctx} Proaktif finansal analiz yap: sağlık skorum, kritik bulgularım ve acil dikkat etmem gerekenler neler?`, sid);
    }, 1400);
  }
  async function loadDemo(scenario) {
    const sid = sessionId || crypto.randomUUID();
    if (!sessionId) setSessionId(sid);
    setChatView('chat');
    
    let url;
    if (!scenario) {
      const randomSector = SECTORS[Math.floor(Math.random() * SECTORS.length)].id;
      const patterns = ['stable', 'growth', 'crisis', 'seasonal', 'recovery', 'mature'];
      const randomPattern = patterns[Math.floor(Math.random() * patterns.length)];
      setUpStatus({
        state: 'loading',
        text: `⏳ Üretiliyor (${randomPattern})...`
      });
      url = `${API}/api/demo?session_id=${sid}&sector=${randomSector}&pattern=${randomPattern}&generate=1`;
    } else {
      setUpStatus({
        state: 'loading',
        text: '⏳ Senaryo yükleniyor...'
      });
      url = `${API}/api/demo?session_id=${sid}&scenario=${scenario}`;
    }
    try {
      const res = await fetch(url, { headers: getAuthHeaders(false) });
      const data = await res.json();
      if (!res.ok) throw new Error(data.detail || 'Hata');
      const label = data.message?.split(' verisi')[0] || 'Senaryo';
      setUpStatus({
        state: 'ok',
        text: `✅ ${label} yüklendi`
      });
      setScenarioName(label);
      setLastAnalysis(new Date());
      handleUpload(data, label, sid);
    } catch (e) {
      setUpStatus({
        state: 'err',
        text: `❌ ${e.message}`
      });
    }
  }
  async function handleFile(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    setUpStatus({
      state: 'loading',
      text: '⏳ Yükleniyor...'
    });
    setChatView('chat');
    const sid = sessionId || crypto.randomUUID();
    if (!sessionId) setSessionId(sid);
    try {
      const headers = getAuthHeaders(false);
      const isXlsx = file.name.toLowerCase().endsWith('.xlsx');
      const endpoint = isXlsx ? 'xlsx' : 'csv';
      const body = isXlsx ? await file.arrayBuffer() : await file.text();
      headers['Content-Type'] = isXlsx ? 'application/octet-stream' : 'text/csv';
      
      const res = await fetch(`${API}/api/upload/${endpoint}?session_id=${sid}&income_column=gelir&expense_column=gider&date_column=tarih`, {
        method: 'POST',
        headers,
        body
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.detail || 'Hata');
      setUpStatus({
        state: 'ok',
        text: `✅ ${data.rows} kayıt yüklendi`
      });
      setScenarioName(file.name);
      setLastAnalysis(new Date());
      handleUpload(data, file.name, sid);
    } catch (e) {
      setUpStatus({
        state: 'err',
        text: `❌ ${e.message}`
      });
    }
  }
  async function send(text, explicitSid) {
    if (!text.trim() || loading) return;
    setInput('');
    setLoading(true);
    if (explicitSid === undefined) setChatView('chat');
    addMsg('user', text);
    const ctrl = new AbortController();
    const tid = setTimeout(() => ctrl.abort(), 90000);
    const useSid = explicitSid !== undefined ? explicitSid : sessionId;
    try {
      const res = await fetch(`${API}/api/chat`, {
        method: 'POST',
        headers: getAuthHeaders(),
        body: JSON.stringify({
          message: text,
          session_id: useSid
        }),
        signal: ctrl.signal
      });
      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error?.message || data.detail || (res.status === 429 ? 'RATE_LIMITED' : 'Sunucu hatası'));
      }
      if (!sessionId) setSessionId(data.session_id);
      const m = extractMetrics(data.reply);
      if (m) setMetrics(m);
      const cf = extractCashflow(data.reply);
      if (cf) setForecast(cf);
      const inc = extractIncentives(data.reply);
      if (inc?.length) {
        setIncentives(prev => {
          const ex = new Set(prev.map(i => i.isim));
          return [...inc.filter(i => !ex.has(i.isim)), ...prev].slice(0, 8);
        });
      }
      addMsg('bot', stripBlocks(data.reply), {
        tools: data.tools_used,
        latency: data.latency_ms,
        agentTrace: data.agent_trace
      });
    } catch (e) {
      const msg = e.name === 'AbortError'
        ? '⏱️ İstek zaman aşımına uğradı (90s). Tekrar deneyin.'
        : e.message;
      addMsg('bot', msg);
    } finally {
      clearTimeout(tid);
      setLoading(false);
    }
  }
  function exportPDF() {
    if (!sessionId) {
      addMsg('bot', '⚠️ PDF için önce veri yükleyin.');
      setChatView('chat');
      return;
    }
    addMsg('bot', '📄 Detay raporu indiriliyor...');
    setChatView('chat');
    
    const a = document.createElement('a');
    a.href = `${API}/api/export/pdf?session_id=${sessionId}&download=1`;
    a.download = `AeraCFO_Rapor_${new Date().toLocaleDateString('tr-TR').replace(/\//g, '-').replace(/\./g, '-')}.pdf`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    
    setTimeout(() => {
      addMsg('bot', '✅ Detay raporu indirildi — muhasebeci, yatırımcı veya banka için hazır.');
    }, 2000);
  }
  const topbarTitle = {
    chat: 'AERA Panel',
    dashboard: 'Finansal \xD6zet',
    incentives: 'Te\u015Fvik \u0026 Hibe',
    whatif: 'What-If Sim\xFClat\xF6r\xFC'
  }[chatView];
  if (appView === 'landing') return /*#__PURE__*/React.createElement(LandingPage, {
    onEnter: () => setAppView('app')
  });
  if (appView === 'booting') return /*#__PURE__*/React.createElement(BootScreen, {
    onComplete: () => setAppView('app')
  });
  return /*#__PURE__*/React.createElement("div", {
    className: "v2app"
  }, /*#__PURE__*/React.createElement(Sidebar, {
    view: chatView,
    setView: setChatView,
    onDemo: loadDemo,
    onUpload: handleFile,
    uploadStatus: upStatus,
    fileRef: fileRef,
    incentives: incentives,
    serverOk: serverOk,
    sessionId: sessionId,
    onExportPDF: exportPDF,
    onSettings: () => setShowSett(true),
    loading: loading
  }), /*#__PURE__*/React.createElement("main", {
    className: "v2main"
  }, /*#__PURE__*/React.createElement("div", {
    className: "mesh-bg",
    "aria-hidden": "true"
  }), /*#__PURE__*/React.createElement("div", {
    className: "ambient-bg",
    "aria-hidden": "true"
  }), /*#__PURE__*/React.createElement("div", {
    className: "topbar"
  }, /*#__PURE__*/React.createElement("div", {
    className: "topbar-left"
  }, /*#__PURE__*/React.createElement("span", {
    className: "topbar-title"
  }, topbarTitle), scenarioName && /*#__PURE__*/React.createElement("span", {
    className: "topbar-sub"
  }, scenarioName, lastAnalysis && (' · ' + lastAnalysis.toLocaleTimeString('tr-TR', { hour: '2-digit', minute: '2-digit' })))), /*#__PURE__*/React.createElement("div", {
    style: { display: 'flex', gap: '8px', alignItems: 'center' }
  }, metrics?.healthSkor != null ? /*#__PURE__*/React.createElement("div", {
    className: "health-pill"
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontWeight: 700,
      fontFamily: 'var(--mono)',
      color: scoreColor(metrics.healthSkor)
    }
  }, metrics.healthEmoji, " ", metrics.healthSkor, "/100"), /*#__PURE__*/React.createElement("span", {
    style: {
      fontWeight: 800,
      fontSize: 14,
    }
  }, metrics.healthHarf)) : null, metrics?.sirketIsmi ? /*#__PURE__*/React.createElement("div", {
    className: "status-pill"
  }, /*#__PURE__*/React.createElement("span", {
    className: "status-dot on",
    style: { background: 'var(--green)' }
  }), " ", metrics.sirketIsmi) : null), /*#__PURE__*/React.createElement("button", {
    className: "settings-btn",
    onClick: () => setShowSett(true),
    title: "G\xF6r\xFCn\xFCm Ayarlar\u0131"
  }, IcSettings(15))), chatView === 'chat' && /*#__PURE__*/React.createElement(ChatView, {
    messages: messages,
    loading: loading,
    input: input,
    setInput: setInput,
    onSend: send,
    metrics: metrics,
    monthlyData: monthly,
    forecast: forecast,
    onUploadClick: () => fileRef.current?.click()
  }), chatView === 'dashboard' && /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      overflowY: 'auto',
      padding: '18px 26px'
    }
  }, /*#__PURE__*/React.createElement(DashboardView, {
    metrics: metrics,
    monthlyData: monthly,
    onDemo: loadDemo,
    setView: setChatView
  })), chatView === 'incentives' && /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      overflowY: 'auto',
      padding: '18px 26px'
    }
  }, /*#__PURE__*/React.createElement(IncentivesView, {
    incentives: incentives,
    setView: setChatView,
    onSend: send
  })), chatView === 'whatif' && /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      overflowY: 'auto',
      padding: '18px 26px'
    }
  }, /*#__PURE__*/React.createElement(WhatIfView, {
    sessionId: sessionId,
    addMsg: addMsg,
    setMainView: setChatView
  }))), showSett && /*#__PURE__*/React.createElement(SettingsPanel, {
    accent: accent,
    setAccent: setAccent,
    bgAnim: bgAnim,
    setBgAnim: setBgAnim,
    theme: theme,
    setTheme: setTheme,
    apiKey: apiKey,
    setApiKey: setApiKey,
    onClose: () => setShowSett(false)
  }));
}
ReactDOM.createRoot(document.getElementById('root')).render(/*#__PURE__*/React.createElement(ErrorBoundary, null, /*#__PURE__*/React.createElement(App, null)));

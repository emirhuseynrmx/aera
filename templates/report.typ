#set page(
  paper: "a4",
  margin: (top: 1.8cm, bottom: 2cm, left: 2cm, right: 2cm),
  fill: rgb("#0f172a"),
)
#set text(size: 9.5pt, lang: "tr", fill: rgb("#e2e8f0"))
#set par(justify: true, leading: 0.65em)
#show heading: it => [#v(8pt)#text(fill: rgb("#f1f5f9"))[#it]#v(4pt)]

// ── Header ──
#block(fill: rgb("#1e293b"), width: 100%, inset: (x: 16pt, y: 14pt), radius: 8pt, stroke: 0.5pt + rgb("#334155"))[
  #grid(columns: (1fr, auto),
    [
      #text(size: 20pt, weight: "bold", fill: white)[AeraCFO]
      #h(8pt)
      #text(size: 9pt, fill: rgb("#00d4ff"))[Yapay Zekâ Destekli CFO Platformu]
      #linebreak()
      #text(size: 7.5pt, fill: rgb("#64748b"))[Kurumsal Finansal Sağlık Raporu v2.0]
    ],
    [
      #align(right)[
        #text(size: 7.5pt, fill: rgb("#94a3b8"))[Dönem: {{DATE_RANGE}}]
        #linebreak()
        #text(size: 7.5pt, fill: rgb("#94a3b8"))[Rapor: {{NOW}}]
      ]
    ],
  )
]

#v(12pt)

= Yönetici Özeti

#block(fill: rgb("#1e293b"), stroke: 0.5pt + rgb("#334155"), radius: 6pt, inset: (x:14pt, y:12pt), width: 100%)[
  Analiz döneminde işletmenin toplam geliri *{{TOTAL_GELIR}} TL*, toplam gideri *{{TOTAL_GIDER}} TL* olarak gerçekleşmiştir. Net nakit akışı #text(fill: rgb("{{NET_COLOR}}"))[*{{NET_SIGN}}{{NET}} TL*] seviyesindedir. Finansal sağlık skoru *{{SKOR}}/100 ({{HARF}})*\ olarak hesaplanmış olup nakit ömrü *{{RUNWAY}}* olarak belirlenmiştir. Genel trend: #text(weight: "bold")[{{TREND}}].
]

#v(12pt)

= Temel Performans Göstergeleri

#grid(columns: (1fr, 1fr, 1fr, 1fr), gutter: 8pt,
{{KPI_CARDS}}
)

#v(12pt)

= Finansal Özet

#grid(columns: (1fr, 1fr), gutter: 12pt,
  [
    #text(size: 8pt, weight: "bold", fill: rgb("#94a3b8"))[Kâr & Zarar Özeti]
    #v(4pt)
    #table(
      columns: (1fr, auto),
      inset: (x: 8pt, y: 6pt),
      stroke: 0.4pt + rgb("#334155"),
      fill: (_, row) => if row == 0 { rgb("#1e293b") } else { rgb("#0f172a") },
      table.header(
        [#text(fill: rgb("#94a3b8"), weight: "bold")[Kalem]],
        [#text(fill: rgb("#94a3b8"), weight: "bold")[Tutar]]
      ),
      [Toplam Gelir], [#text(fill: rgb("#16a34a"))[{{TOTAL_GELIR_F2}} TL]],
      [Toplam Gider], [#text(fill: rgb("#dc2626"))[{{TOTAL_GIDER_F2}} TL]],
      [Net Sonuç], [#text(fill: rgb("{{NET_COLOR}}"), weight: "bold")[{{NET_SIGN}}{{NET_F2}} TL]],
      [Aylık Ortalama Gelir], [{{MONTHLY_GELIR_F2}} TL],
      [Aylık Ortalama Gider], [{{MONTHLY_GIDER_F2}} TL],
    )
  ],
  [
    #text(size: 8pt, weight: "bold", fill: rgb("#94a3b8"))[Risk & Sağlık Özeti]
    #v(4pt)
    #table(
      columns: (1fr, auto),
      inset: (x: 8pt, y: 6pt),
      stroke: 0.4pt + rgb("#334155"),
      fill: (_, row) => if row == 0 { rgb("#1e293b") } else { rgb("#0f172a") },
      table.header(
        [#text(fill: rgb("#94a3b8"), weight: "bold")[Gösterge]],
        [#text(fill: rgb("#94a3b8"), weight: "bold")[Değer]]
      ),
      [Finansal Sağlık Skoru], [#text(fill: rgb("{{SKOR_COLOR}}"), weight: "bold")[{{SKOR}}/100 ({{HARF}})]],
      [Risk Seviyesi], [#text(fill: rgb("{{RISK_COLOR}}"), weight: "bold")[{{RISK_LABEL}}]],
      [Nakit Ömrü], [#text(weight: "bold")[{{RUNWAY}}]],
      [Dönem Trendi], [{{TREND}}],
      [Gelir/Gider Oranı], [{{GG_RATIO}}x],
    )
  ],
)

#v(12pt)

= Aylık Gelir / Gider Analizi

#table(
  columns: (auto, 1fr, 1fr, 1fr, auto),
  inset: (x: 8pt, y: 6pt),
  stroke: 0.4pt + rgb("#334155"),
  fill: (_, row) => if row == 0 { rgb("#00d4ff").darken(70%) } else if calc.odd(row) { rgb("#1e293b") } else { rgb("#0f172a") },
  table.header(
    [#text(fill: white, weight: "bold")[Ay]],
    [#text(fill: white, weight: "bold")[Gelir (TL)]],
    [#text(fill: white, weight: "bold")[Gider (TL)]],
    [#text(fill: white, weight: "bold")[Net (TL)]],
    [#text(fill: white, weight: "bold")[Durum]],
  ),
{{MONTHLY_ROWS}}
)

#v(12pt)

= Analiz ve Tavsiyeler

#block(fill: rgb("#16a34a").darken(80%), stroke: 0.5pt + rgb("#16a34a").darken(40%), radius: 6pt, inset: (x:14pt,y:12pt), width: 100%)[
  #text(size: 8pt, weight: "bold", fill: rgb("#86efac"))[Muhasebeci / Yatırımcı İçin Not:]

  {{TAVSIYE}}
]

#v(8pt)

#line(length: 100%, stroke: 0.4pt + rgb("#334155"))
#v(4pt)
#grid(columns: (1fr, auto),
  [
    #text(size: 6.5pt, fill: rgb("#64748b"))[
      Bu rapor AeraCFO yapay zekâ sistemi tarafından otomatik olarak üretilmiştir. Yatırım kararları için uzman danışmanlık alınması önerilir. Gizlilik: Bu belge yalnızca alıcısına yöneliktir.
    ]
  ],
  [
    #align(right)[#text(size: 6.5pt, fill: rgb("#64748b"))[
      AeraCFO v0.1.0 | Rust/Axum + Polars\
      Gemini 2.5 Flash | BUSL-1.1
    ]]
  ],
)

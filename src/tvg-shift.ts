export const TVG_SHIFTS: { hours: number; label: string }[] = [
  { hours: 0, label: "0    ·  GMT−5  Eastern — New York, Toronto, Bogotá" },
  { hours: -1, label: "−1   ·  GMT−6  Central — Chicago, Mexico City, Winnipeg" },
  { hours: -2, label: "−2   ·  GMT−7  Mountain — Denver, Phoenix, Calgary" },
  { hours: -3, label: "−3   ·  GMT−8  Pacific — Los Angeles, Vancouver, Tijuana" },
  { hours: -4, label: "−4   ·  GMT−9  Alaska — Anchorage" },
  { hours: -5, label: "−5   ·  GMT−10 Hawaii — Honolulu, Tahiti" },
  { hours: -6, label: "−6   ·  GMT−11 Samoa, Midway" },
  { hours: -7, label: "−7   ·  GMT−12 Baker Island" },
  { hours: 1, label: "+1   ·  GMT−4  Atlantic — Halifax, Santo Domingo, La Paz" },
  { hours: 2, label: "+2   ·  GMT−3  São Paulo, Buenos Aires, Montevideo" },
  { hours: 3, label: "+3   ·  GMT−2  South Georgia / mid-Atlantic" },
  { hours: 4, label: "+4   ·  GMT−1  Azores, Cape Verde" },
  { hours: 5, label: "+5   ·  GMT+0  UTC — London, Lisbon, Reykjavik, Accra" },
  { hours: 6, label: "+6   ·  GMT+1  CET — Paris, Berlin, Rome, Lagos, Madrid" },
  { hours: 7, label: "+7   ·  GMT+2  EET — Cairo, Athens, Johannesburg, Helsinki" },
  { hours: 8, label: "+8   ·  GMT+3  Moscow, Istanbul, Riyadh, Nairobi, Kuwait" },
  { hours: 9, label: "+9   ·  GMT+4  Dubai, Baku, Tbilisi, Mauritius" },
  { hours: 10, label: "+10  ·  GMT+5  Pakistan (PKT), Maldives, Yekaterinburg" },
  { hours: 10.5, label: "+10.5 ·  GMT+5:30 India (IST) — Mumbai, Delhi, Colombo" },
  { hours: 11, label: "+11  ·  GMT+6  Bangladesh, Almaty, Omsk, Bhutan" },
  { hours: 11.5, label: "+11.5 ·  GMT+6:30 Myanmar, Cocos Islands" },
  { hours: 12, label: "+12  ·  GMT+7  Bangkok, Jakarta, Ho Chi Minh, Hanoi" },
  { hours: 13, label: "+13  ·  GMT+8  China, Singapore, Hong Kong, Perth, Manila" },
  { hours: 14, label: "+14  ·  GMT+9  Japan (JST), Korea (KST), Yakutsk" },
  { hours: 14.5, label: "+14.5 ·  GMT+9:30 Adelaide, Darwin (ACST)" },
  { hours: 15, label: "+15  ·  GMT+10 Sydney, Melbourne, Brisbane, Guam" },
  { hours: 16, label: "+16  ·  GMT+11 Magadan, Solomon Islands, New Caledonia" },
  { hours: 17, label: "+17  ·  GMT+12 Auckland, Fiji, Kamchatka, Marshall Islands" },
  { hours: 18, label: "+18  ·  GMT+13 Tonga, Samoa (DST), Phoenix Islands" },
];

export function fillTvgShiftSelect(sel: HTMLSelectElement, selectedHours = 0): void {
  sel.innerHTML = "";
  for (const z of TVG_SHIFTS) {
    const o = document.createElement("option");
    o.value = String(z.hours);
    o.textContent = z.label;
    if (z.hours === selectedHours) o.selected = true;
    sel.appendChild(o);
  }
}

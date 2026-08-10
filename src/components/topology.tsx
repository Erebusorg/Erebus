const LAYERS = [
  { name: "Entry", nodes: 4 },
  { name: "Relay", nodes: 4 },
  { name: "Exit", nodes: 4 },
];

export function Topology() {
  const width = 720;
  const height = 300;
  const colX = (l: number) => 140 + l * 200;
  const rowY = (n: number, total: number) =>
    60 + (n * (height - 120)) / (total - 1);

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="h-auto w-full"
      role="img"
      aria-label="Three layers of mix nodes: entry, relay, exit"
    >
      <g stroke="var(--line)" strokeWidth="1">
        {LAYERS.slice(0, -1).flatMap((layer, l) =>
          Array.from({ length: layer.nodes }).flatMap((_, a) =>
            Array.from({ length: LAYERS[l + 1].nodes }).map((_, b) => (
              <line
                key={`${l}-${a}-${b}`}
                x1={colX(l)}
                y1={rowY(a, layer.nodes)}
                x2={colX(l + 1)}
                y2={rowY(b, LAYERS[l + 1].nodes)}
              />
            )),
          ),
        )}
      </g>

      <g stroke="var(--line)" strokeWidth="1">
        {Array.from({ length: LAYERS[0].nodes }).map((_, n) => (
          <line
            key={`in-${n}`}
            x1="30"
            y1={height / 2}
            x2={colX(0)}
            y2={rowY(n, LAYERS[0].nodes)}
          />
        ))}
        {Array.from({ length: LAYERS[2].nodes }).map((_, n) => (
          <line
            key={`out-${n}`}
            x1={colX(2)}
            y1={rowY(n, LAYERS[2].nodes)}
            x2={width - 30}
            y2={height / 2}
          />
        ))}
      </g>

      <path
        d={`M30 ${height / 2} L${colX(0)} ${rowY(1, 4)} L${colX(1)} ${rowY(3, 4)} L${colX(2)} ${rowY(0, 4)} L${width - 30} ${height / 2}`}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1.5"
        strokeDasharray="6 8"
        style={{ animation: "dash 4s linear infinite" }}
      />

      {LAYERS.map((layer, l) => (
        <g key={layer.name}>
          <text
            x={colX(l)}
            y="28"
            textAnchor="middle"
            className="fill-muted font-mono"
            fontSize="11"
            letterSpacing="2"
          >
            {layer.name.toUpperCase()}
          </text>
          {Array.from({ length: layer.nodes }).map((_, n) => (
            <circle
              key={n}
              cx={colX(l)}
              cy={rowY(n, layer.nodes)}
              r="4.5"
              fill="var(--background)"
              stroke="var(--foreground)"
              strokeWidth="1"
              style={{
                animation: `pulse-node ${3 + ((l + n) % 3)}s ease-in-out ${(l + n) * 0.3}s infinite`,
              }}
            />
          ))}
        </g>
      ))}

      <g className="fill-muted font-mono" fontSize="11" letterSpacing="2">
        <text x="30" y={height / 2 - 14} textAnchor="middle">
          YOU
        </text>
        <text x={width - 30} y={height / 2 - 14} textAnchor="middle">
          CHAIN
        </text>
      </g>
      <circle cx="30" cy={height / 2} r="4.5" fill="var(--accent)" />
      <circle cx={width - 30} cy={height / 2} r="4.5" fill="var(--accent)" />
    </svg>
  );
}

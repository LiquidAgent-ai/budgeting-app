"use client";

import {
  ResponsiveContainer,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
} from "recharts";
import { money } from "@/lib/format";

export function SpendChart({ data }: { data: { category: string; amount: number }[] }) {
  if (!data.length) {
    return <p className="text-sm text-ink-soft/70">No spending this month yet.</p>;
  }

  return (
    <div className="h-64 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid stroke="rgba(12,47,42,0.08)" vertical={false} />
          <XAxis
            dataKey="category"
            tick={{ fill: "#1a4a42", fontSize: 11 }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            tick={{ fill: "#1a4a42", fontSize: 11 }}
            axisLine={false}
            tickLine={false}
            tickFormatter={(v) => `$${v}`}
          />
          <Tooltip
            formatter={(value) => money(Number(value ?? 0))}
            contentStyle={{
              borderRadius: 12,
              border: "1px solid rgba(12,47,42,0.12)",
              background: "#fbfaf7",
            }}
          />
          <Bar dataKey="amount" fill="#1f7a6c" radius={[8, 8, 0, 0]} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

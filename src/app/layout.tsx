import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";
import { Nav } from "@/components/nav";
import { Footer } from "@/components/footer";

const sans = Inter({
  variable: "--font-sans-custom",
  subsets: ["latin"],
});

const mono = JetBrains_Mono({
  variable: "--font-mono-custom",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  // Needed to turn opengraph-image.png into the absolute URL crawlers require.
  metadataBase: new URL(
    process.env.NEXT_PUBLIC_SITE_URL ?? "http://localhost:3000",
  ),
  title: "Erebus — network-layer privacy for tokenized finance",
  description:
    "Erebus hides who you are, what you trade, and how you pay. A Sphinx mixnet, shielded fee payments, and private reads for tokenized equities on Robinhood Chain.",
  openGraph: {
    title: "Erebus",
    description:
      "Network-layer privacy for tokenized finance. Nobody learns your address, your positions, or your timing.",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    site: "@Erebusorg",
    creator: "@Erebusorg",
  },
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      className={`${sans.variable} ${mono.variable} h-full antialiased`}
    >
      <body className="grain min-h-full flex flex-col">
        <Nav />
        <main className="flex-1">{children}</main>
        <Footer />
      </body>
    </html>
  );
}

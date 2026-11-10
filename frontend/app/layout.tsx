import type { Metadata } from "next";
import { Caveat, DM_Sans } from "next/font/google";
import { Providers } from "./providers";
import "./globals.css";

// DM Sans carries the whole interface; Caveat is used sparingly, only for handwritten callouts.
const dmSans = DM_Sans({
  variable: "--font-dm-sans",
  subsets: ["latin"],
  weight: ["400", "500", "700", "800"],
});

const caveat = Caveat({
  variable: "--font-caveat",
  subsets: ["latin"],
  weight: ["600", "700"],
});

export const metadata: Metadata = {
  title: "herit — inheritance gated by proof of life",
  description:
    "Open a keyless estate on NEAR, prove you're alive with a World ID Selfie Check, and your heirs' soulbound credentials wake up when you stop.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={`${dmSans.variable} ${caveat.variable} h-full`}>
      <body className="flex min-h-full flex-col bg-cream text-ink">
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}

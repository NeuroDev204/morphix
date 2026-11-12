import Header from "@/components/Header";
import Hero from "@/components/Hero";
import MarqueeTicker from "@/components/MarqueeTicker";
import TrustMetrics from "@/components/TrustMetrics";
import DropzonePlayground from "@/components/DropzonePlayground";
import StorageCalculator from "@/components/StorageCalculator";
import FeaturesZPattern from "@/components/FeaturesZPattern";
import CliDoctorTerminal from "@/components/CliDoctorTerminal";
import ComparisonTable from "@/components/ComparisonTable";
import FaqAccordion from "@/components/FaqAccordion";
import DownloadMatrix from "@/components/DownloadMatrix";
import Footer from "@/components/Footer";

export default function Home() {
  return (
    <div className="min-h-screen flex flex-col bg-background selection:bg-teal-100 selection:text-teal-900">
      <Header />
      <main className="flex-1">
        <Hero />
        <MarqueeTicker />
        <TrustMetrics />
        <DropzonePlayground />
        <StorageCalculator />
        <FeaturesZPattern />
        <CliDoctorTerminal />
        <ComparisonTable />
        <FaqAccordion />
        <DownloadMatrix />
      </main>
      <Footer />
    </div>
  );
}

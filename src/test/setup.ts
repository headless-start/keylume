import "@testing-library/jest-dom/vitest";

// jsdom lacks canvas and IntersectionObserver; stub just enough for components.
HTMLCanvasElement.prototype.getContext = (() => null) as unknown as HTMLCanvasElement["getContext"];
class IO { observe() {} disconnect() {} unobserve() {} takeRecords() { return []; } }
(globalThis as unknown as { IntersectionObserver: unknown }).IntersectionObserver = IO;

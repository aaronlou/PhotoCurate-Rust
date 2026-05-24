/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        sidebar: "#f5f5f7",
        "sidebar-dark": "#1e1e1e",
      },
    },
  },
  plugins: [],
};

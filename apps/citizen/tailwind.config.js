/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      keyframes: {
        "pulse-slow": {
          "0%, 100%": { opacity: 0.4, transform: "scale(1)" },
          "50%": { opacity: 0.7, transform: "scale(1.05)" },
        },
        "pulse-slower": {
          "0%, 100%": { opacity: 0.3, transform: "scale(1)" },
          "50%": { opacity: 0.5, transform: "scale(1.08)" },
        },
      },
      animation: {
        "pulse-slow": "pulse-slow 6s ease-in-out infinite",
        "pulse-slower": "pulse-slower 8s ease-in-out infinite",
      },
    },
  },
  plugins: [],
};

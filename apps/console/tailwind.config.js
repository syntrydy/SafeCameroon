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
        float: {
          "0%, 100%": { transform: "translateY(0) translateX(0)" },
          "25%": { transform: "translateY(-10px) translateX(5px)" },
          "50%": { transform: "translateY(-5px) translateX(-5px)" },
          "75%": { transform: "translateY(-15px) translateX(3px)" },
        },
        "spin-very-slow": {
          from: { transform: "rotate(0deg)" },
          to: { transform: "rotate(360deg)" },
        },
        "spin-reverse": {
          from: { transform: "rotate(360deg)" },
          to: { transform: "rotate(0deg)" },
        },
      },
      animation: {
        "pulse-slow": "pulse-slow 6s ease-in-out infinite",
        "pulse-slower": "pulse-slower 8s ease-in-out infinite",
        float: "float 8s ease-in-out infinite",
        "spin-very-slow": "spin-very-slow 30s linear infinite",
        "spin-reverse": "spin-reverse 25s linear infinite",
      },
    },
  },
  plugins: [],
};

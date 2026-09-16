import { Navigate, Outlet, Route, Routes } from "react-router-dom";

import { AppLayout } from "./components/AppLayout";
import { RequireAuth } from "./auth/RequireAuth";
import { AlertList } from "./pages/alerts/AlertList";
import { AlertPreview } from "./pages/alerts/AlertPreview";
import { CaseDetail } from "./pages/case-detail/CaseDetail";
import { Login } from "./pages/Login";
import { ReviewQueue } from "./pages/review-queue/ReviewQueue";
import { Subscriptions } from "./pages/subscriptions/Subscriptions";

function ProtectedLayout() {
  return (
    <RequireAuth>
      <AppLayout>
        <Outlet />
      </AppLayout>
    </RequireAuth>
  );
}

export function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route element={<ProtectedLayout />}>
        <Route path="/" element={<ReviewQueue />} />
        <Route path="/cases/:id" element={<CaseDetail />} />
        <Route path="/alerts" element={<AlertList />} />
        <Route path="/alerts/:id" element={<AlertPreview />} />
        <Route path="/subscriptions" element={<Subscriptions />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

import { useEffect } from "react";
import { Navigate, Outlet, Route, Routes } from "react-router-dom";

import { AppLayout } from "./components/AppLayout";
import { RequireAuth } from "./auth/RequireAuth";
import { RequireOrgAdmin } from "./auth/RequireOrgAdmin";
import { RequirePlatformAdmin } from "./auth/RequirePlatformAdmin";
import { BRAND_NAME } from "./config/brand";
import { AlertList } from "./pages/alerts/AlertList";
import { AlertPreview } from "./pages/alerts/AlertPreview";
import { CaseDetail } from "./pages/case-detail/CaseDetail";
import { CaseList } from "./pages/case-detail/CaseList";
import { DeliveryDetail } from "./pages/deliveries/DeliveryDetail";
import { DeliveryList } from "./pages/deliveries/DeliveryList";
import { Login } from "./pages/Login";
import { MyOrganization } from "./pages/organization/MyOrganization";
import { Organizations } from "./pages/organizations/Organizations";
import { ReviewQueue } from "./pages/review-queue/ReviewQueue";
import { Subscriptions } from "./pages/subscriptions/Subscriptions";
import { LanguageProvider } from "./i18n/LanguageContext";

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
  useEffect(() => {
    document.title = `${BRAND_NAME} Portal`;
  }, []);

  return (
    <LanguageProvider>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route element={<ProtectedLayout />}>
          <Route path="/" element={<ReviewQueue />} />
          <Route path="/cases" element={<CaseList />} />
          <Route path="/cases/:id" element={<CaseDetail />} />
          <Route path="/alerts" element={<AlertList />} />
          <Route path="/alerts/:id" element={<AlertPreview />} />
          <Route path="/alerts/:id/deliveries" element={<DeliveryList />} />
          <Route path="/deliveries/:id" element={<DeliveryDetail />} />
          <Route path="/subscriptions" element={<Subscriptions />} />
          <Route
            path="/my-organization"
            element={
              <RequireOrgAdmin>
                <MyOrganization />
              </RequireOrgAdmin>
            }
          />
          <Route
            path="/organizations"
            element={
              <RequirePlatformAdmin>
                <Organizations />
              </RequirePlatformAdmin>
            }
          />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </LanguageProvider>
  );
}

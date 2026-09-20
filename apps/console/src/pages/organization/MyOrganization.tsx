import { useCallback, useEffect, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";

import { registerMember } from "../../api/auth";
import { ApiError } from "../../api/client";
import { getOrganization, listMembers, updateOrganizationProfile, type Member, type Organization } from "../../api/organizations";
import {
  getDeliveryPreference,
  listSubscriptionsForConsumer,
  type DeliveryPreference,
  type Subscription,
} from "../../api/subscriptions";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";
import { CreateSubscriptionForm } from "../subscriptions/CreateSubscriptionForm";
import { DeliveryPreferenceForm } from "../subscriptions/DeliveryPreferenceForm";
import { SubscriptionCard } from "../subscriptions/SubscriptionCard";

/** A reviewer's own-organization view: read-only org detail (trust grants
 * are the platform's decision, not the org's own — see
 * pages/organizations/Organizations.tsx) and member list, scoped to this
 * org by the backend (`require_membership_of` in
 * apps/api/src/organizations.rs) — any `MEMBER`/`ORG_ADMIN`/`PLATFORM_ADMIN`
 * of the org may see it. The invite form is further restricted to an
 * `ORG_ADMIN`/`PLATFORM_ADMIN` (`require_platform_admin_or_org_admin_of`),
 * since a plain member may not grant membership. */
export function MyOrganization() {
  const { t } = useTranslation();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>/<RequireOrgMember>.
  const token = session!.token;
  const organizationId = session!.organizationId;
  const canInviteMembers = session!.role === "ORG_ADMIN" || session!.role === "PLATFORM_ADMIN";

  const [organization, setOrganization] = useState<Organization | null>(null);
  const [members, setMembers] = useState<Member[] | null>(null);
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [deliveryPreference, setDeliveryPreference] = useState<DeliveryPreference | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [description, setDescription] = useState("");
  const [location, setLocation] = useState("");
  const [contact, setContact] = useState("");
  const [savingProfile, setSavingProfile] = useState(false);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [profileSaved, setProfileSaved] = useState(false);

  const [inviteEmail, setInviteEmail] = useState("");
  const [inviting, setInviting] = useState(false);
  const [inviteError, setInviteError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!organizationId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setLoadError(null);
    try {
      const [org, orgMembers] = await Promise.all([
        getOrganization(token, organizationId),
        listMembers(token, organizationId),
      ]);
      setOrganization(org);
      setDescription(org.description ?? "");
      setLocation(org.location ?? "");
      setContact(org.contact ?? "");
      setMembers(orgMembers);

      if (org.consumer_id) {
        try {
          setSubscriptions(await listSubscriptionsForConsumer(token, org.consumer_id));
        } catch (cause) {
          setLoadError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
        }
        try {
          setDeliveryPreference(await getDeliveryPreference(token, org.consumer_id));
        } catch (cause) {
          if (cause instanceof ApiError && cause.code === "DELIVERY_PREFERENCE_NOT_FOUND") {
            setDeliveryPreference(null);
          } else {
            setLoadError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
          }
        }
      }
    } catch (cause) {
      setLoadError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }, [token, organizationId, t]);

  useEffect(() => {
    void load();
  }, [load]);

  async function saveProfile(event: FormEvent) {
    event.preventDefault();
    if (!organizationId) return;
    setSavingProfile(true);
    setProfileError(null);
    setProfileSaved(false);
    try {
      setOrganization(await updateOrganizationProfile(token, organizationId, description, location, contact));
      setProfileSaved(true);
    } catch (cause) {
      setProfileError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally { setSavingProfile(false); }
  }

  async function handleInvite(event: FormEvent) {
    event.preventDefault();
    if (!organizationId) {
      return;
    }
    setInviting(true);
    setInviteError(null);
    try {
      await registerMember(token, inviteEmail.trim(), organizationId);
      setInviteEmail("");
      await load();
    } catch (cause) {
      setInviteError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setInviting(false);
    }
  }

  if (!organizationId) {
    return (
      <div>
        <h1 className="mb-4 text-2xl font-semibold text-white">{t.myOrganization.heading}</h1>
        <p className="mb-3 text-sm text-slate-400">{t.myOrganization.noOrganization}</p>
        <Link to="/organizations" className="text-sm text-emerald-400 hover:underline">
          {t.myOrganization.goToOrganizations}
        </Link>
      </div>
    );
  }

  return (
    <div>
      <h1 className="mb-6 text-2xl font-semibold text-white">
        {organization ? organization.name : t.myOrganization.heading}
      </h1>

      {loading && <p className="text-sm text-slate-400">{t.myOrganization.loading}</p>}
      {loadError && (
        <p role="alert" className="text-sm text-red-300">
          {loadError}
        </p>
      )}

      {organization && (
        <div className="mb-6 flex flex-col gap-1 text-xs text-slate-400">
          <span>
            {t.organizations.verifiedIncidentTypes}:{" "}
            {organization.verified_incident_types.length > 0
              ? organization.verified_incident_types.map((value) => value.replace(/_/g, " ")).join(", ")
              : t.organizations.none}
          </span>
          <span>
            {t.organizations.verifiedVisibilities}:{" "}
            {organization.verified_alert_visibilities.length > 0
              ? organization.verified_alert_visibilities.join(", ")
              : t.organizations.none}
          </span>
        </div>
      )}

      {organization && canInviteMembers && (
        <form onSubmit={(event) => void saveProfile(event)} className="mb-6 space-y-3 rounded-2xl border border-white/10 p-4">
          <h2 className="text-sm font-semibold text-white">{t.organizations.profileHeading}</h2>
          <label className="block text-sm text-slate-300">
            {t.organizations.descriptionLabel}
            <textarea value={description} onChange={(event) => setDescription(event.target.value)} maxLength={5000} className="mt-1 block w-full rounded bg-slate-800 p-2 text-white" />
          </label>
          <label className="block text-sm text-slate-300">
            {t.organizations.locationLabel}
            <input value={location} onChange={(event) => setLocation(event.target.value)} maxLength={500} className="mt-1 block w-full rounded bg-slate-800 p-2 text-white" />
          </label>
          <label className="block text-sm text-slate-300">
            {t.organizations.contactLabel}
            <input value={contact} onChange={(event) => setContact(event.target.value)} maxLength={1000} className="mt-1 block w-full rounded bg-slate-800 p-2 text-white" />
          </label>
          <button disabled={savingProfile} className="rounded bg-emerald-700 px-4 py-2 text-sm text-white disabled:opacity-50">{t.organizations.saveProfile}</button>
          {profileError && <p role="alert" className="text-red-300">{profileError}</p>}
          {profileSaved && <p role="status" className="text-emerald-300">{t.organizations.profileSaved}</p>}
        </form>
      )}
      {organization && !canInviteMembers && (
        <section className="mb-6 text-sm text-slate-300">
          <p>{organization.description}</p><p>{organization.location}</p><p>{organization.contact}</p>
        </section>
      )}

      {organization && organization.consumer_id && (
        <>
          <section className="mb-6">
            <h2 className="mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
              {t.subscriptions.subscriptionsHeading}
            </h2>
            {subscriptions.map((subscription) => (
              <SubscriptionCard
                key={subscription.subscription_id}
                token={token}
                subscription={subscription}
                onUpdated={(updated) =>
                  setSubscriptions((current) =>
                    current.map((s) => (s.subscription_id === updated.subscription_id ? updated : s)),
                  )
                }
              />
            ))}
            <CreateSubscriptionForm
              token={token}
              consumerId={organization.consumer_id}
              onCreated={(created) => setSubscriptions((current) => [...current, created])}
            />
          </section>

          <section className="mb-6">
            <h2 className="mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
              {t.subscriptions.deliveryPreferenceHeading}
            </h2>
            {!deliveryPreference && (
              <p className="mb-2 text-sm text-slate-500">{t.subscriptions.noDeliveryPreference}</p>
            )}
            <DeliveryPreferenceForm
              token={token}
              consumerId={organization.consumer_id}
              initial={deliveryPreference}
              onSaved={setDeliveryPreference}
            />
          </section>
        </>
      )}

      {canInviteMembers && (
        <form
          onSubmit={(event) => void handleInvite(event)}
          className="mb-6 rounded-2xl border border-white/[0.08] bg-white/[0.03] p-4"
        >
          <h2 className="mb-3 text-sm font-semibold text-white">{t.myOrganization.inviteHeading}</h2>
          <div className="flex flex-wrap items-end gap-2">
            <div className="flex-1">
              <label htmlFor="invite-member-email" className="mb-1 block text-xs text-slate-400">
                {t.myOrganization.emailLabel}
              </label>
              <input
                id="invite-member-email"
                type="email"
                value={inviteEmail}
                placeholder={t.myOrganization.emailPlaceholder}
                onChange={(event) => setInviteEmail(event.target.value)}
                className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
              />
            </div>
            <button
              type="submit"
              disabled={inviting || !inviteEmail.trim()}
              className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-4 py-2 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {inviting ? t.myOrganization.inviting : t.myOrganization.invite}
            </button>
          </div>
          {inviteError && (
            <p role="alert" className="mt-2 text-sm text-red-300">
              {inviteError}
            </p>
          )}
        </form>
      )}

      <h2 className="mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
        {t.organizations.membersHeading}
      </h2>
      {members && members.length === 0 && (
        <p className="text-sm text-slate-500">{t.organizations.noMembers}</p>
      )}
      {members && members.length > 0 && (
        <ul className="space-y-1 text-sm text-slate-300">
          {members.map((member) => (
            <li key={member.reviewer_id} className="flex items-center justify-between gap-3">
              <span>{member.email}</span>
              <span className="text-xs text-slate-500">{member.role}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

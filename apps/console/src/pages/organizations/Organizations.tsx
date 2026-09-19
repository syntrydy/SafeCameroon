import { useCallback, useEffect, useState, type FormEvent } from "react";

import { registerOrgAdmin } from "../../api/auth";
import { ApiError } from "../../api/client";
import type { AlertVisibility } from "../../api/alerts";
import type { IncidentType } from "../../api/cases";
import {
  createOrganization,
  deactivateOrganization,
  listMembers,
  listOrganizations,
  reactivateOrganization,
  setTrustGrants,
  updateOrganizationProfile,
  type Member,
  type Organization,
} from "../../api/organizations";
import {
  getDeliveryPreference,
  listSubscriptionsForConsumer,
  type DeliveryPreference,
  type Subscription,
} from "../../api/subscriptions";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";
import { CreateSubscriptionForm } from "../subscriptions/CreateSubscriptionForm";
import { DeliveryPreferenceForm } from "../subscriptions/DeliveryPreferenceForm";
import { SubscriptionCard } from "../subscriptions/SubscriptionCard";

const INCIDENT_TYPES: IncidentType[] = ["MISSING_CHILD", "OTHER_PROTECTION_INCIDENT"];
const ALERT_VISIBILITIES: AlertVisibility[] = ["INTERNAL", "PARTNER", "COMMUNITY", "PUBLIC"];

function toggleValue<T>(values: T[], value: T): T[] {
  return values.includes(value) ? values.filter((v) => v !== value) : [...values, value];
}

function CreateOrganizationForm({
  token,
  onCreated,
}: {
  token: string;
  onCreated: (organization: Organization) => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [location, setLocation] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const created = await createOrganization(
        token,
        name,
        description.trim() || undefined,
        location.trim() || undefined,
      );
      onCreated(created);
      setName("");
      setDescription("");
      setLocation("");
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form
      onSubmit={(event) => void handleSubmit(event)}
      className="mb-6 rounded-2xl border border-white/[0.08] bg-white/[0.03] p-4"
    >
      <h2 className="mb-3 text-sm font-semibold text-white">{t.organizations.createHeading}</h2>
      <div className="flex flex-wrap items-end gap-2">
        <div className="flex-1">
          <label htmlFor="new-organization-name" className="mb-1 block text-xs text-slate-400">
            {t.organizations.name}
          </label>
          <input
            id="new-organization-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
        </div>
        <button
          type="submit"
          disabled={submitting || !name.trim()}
          className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-4 py-2 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {submitting ? t.organizations.creating : t.organizations.create}
        </button>
      </div>
      <div className="mt-2 flex flex-wrap gap-2">
        <div className="flex-1">
          <label htmlFor="new-organization-description" className="mb-1 block text-xs text-slate-400">
            {t.organizations.descriptionLabel}
          </label>
          <input
            id="new-organization-description"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
        </div>
        <div className="flex-1">
          <label htmlFor="new-organization-location" className="mb-1 block text-xs text-slate-400">
            {t.organizations.locationLabel}
          </label>
          <input
            id="new-organization-location"
            value={location}
            onChange={(event) => setLocation(event.target.value)}
            className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
        </div>
      </div>
      {error && (
        <p role="alert" className="mt-2 text-sm text-red-300">
          {error}
        </p>
      )}
    </form>
  );
}

function TrustGrantSummary({ organization, t }: { organization: Organization; t: Translations }) {
  return (
    <div className="mt-1 flex flex-col gap-1 text-xs text-slate-400">
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
  );
}

function OrganizationProfileForm({
  token,
  organization,
  onUpdated,
}: {
  token: string;
  organization: Organization;
  onUpdated: (organization: Organization) => void;
}) {
  const { t } = useTranslation();
  const [description, setDescription] = useState(organization.description ?? "");
  const [location, setLocation] = useState(organization.location ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const updated = await updateOrganizationProfile(
        token,
        organization.organization_id,
        description.trim(),
        location.trim(),
      );
      onUpdated(updated);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mb-3">
      <h3 className="mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
        {t.organizations.profileHeading}
      </h3>
      <div className="flex flex-wrap gap-2">
        <div className="flex-1">
          <label
            htmlFor={`org-description-${organization.organization_id}`}
            className="mb-1 block text-xs text-slate-400"
          >
            {t.organizations.descriptionLabel}
          </label>
          <input
            id={`org-description-${organization.organization_id}`}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder={t.organizations.noDescription}
            className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
        </div>
        <div className="flex-1">
          <label
            htmlFor={`org-location-${organization.organization_id}`}
            className="mb-1 block text-xs text-slate-400"
          >
            {t.organizations.locationLabel}
          </label>
          <input
            id={`org-location-${organization.organization_id}`}
            value={location}
            onChange={(event) => setLocation(event.target.value)}
            placeholder={t.organizations.noLocation}
            className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
        </div>
      </div>
      {error && (
        <p role="alert" className="mt-2 text-sm text-red-300">
          {error}
        </p>
      )}
      <button
        type="button"
        disabled={saving}
        onClick={() => void handleSave()}
        className="mt-2 rounded-lg border border-white/[0.08] px-3 py-1.5 text-xs font-medium text-slate-300 transition-colors hover:bg-white/[0.05] disabled:cursor-not-allowed disabled:opacity-50"
      >
        {saving ? t.organizations.savingProfile : t.organizations.saveProfile}
      </button>
    </div>
  );
}

function OrganizationContact({ token, organization }: { token: string; organization: Organization }) {
  const { t } = useTranslation();
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [deliveryPreference, setDeliveryPreference] = useState<DeliveryPreference | null>(null);
  const [error, setError] = useState<string | null>(null);

  const consumerId = organization.consumer_id;

  useEffect(() => {
    if (!consumerId) {
      return;
    }
    let cancelled = false;
    async function load() {
      try {
        const result = await listSubscriptionsForConsumer(token, consumerId!);
        if (!cancelled) {
          setSubscriptions(result);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
        }
      }
      try {
        const preference = await getDeliveryPreference(token, consumerId!);
        if (!cancelled) {
          setDeliveryPreference(preference);
        }
      } catch (cause) {
        if (cancelled) {
          return;
        }
        if (cause instanceof ApiError && cause.code === "DELIVERY_PREFERENCE_NOT_FOUND") {
          setDeliveryPreference(null);
        } else {
          setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
        }
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [token, consumerId, t]);

  if (!consumerId) {
    return null;
  }

  return (
    <div className="mb-3">
      <h3 className="mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
        {t.subscriptions.subscriptionsHeading}
      </h3>
      {error && (
        <p role="alert" className="mb-2 text-sm text-red-300">
          {error}
        </p>
      )}
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
        consumerId={consumerId}
        onCreated={(created) => setSubscriptions((current) => [...current, created])}
      />

      <h3 className="mt-4 mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
        {t.subscriptions.deliveryPreferenceHeading}
      </h3>
      {!deliveryPreference && (
        <p className="mb-2 text-sm text-slate-500">{t.subscriptions.noDeliveryPreference}</p>
      )}
      <DeliveryPreferenceForm
        token={token}
        consumerId={consumerId}
        initial={deliveryPreference}
        onSaved={setDeliveryPreference}
      />
    </div>
  );
}

function OrganizationDetail({
  token,
  organization,
  onUpdated,
}: {
  token: string;
  organization: Organization;
  onUpdated: (organization: Organization) => void;
}) {
  const { t } = useTranslation();
  const [members, setMembers] = useState<Member[] | null>(null);
  const [membersError, setMembersError] = useState<string | null>(null);
  const [incidentTypes, setIncidentTypes] = useState<IncidentType[]>(
    organization.verified_incident_types,
  );
  const [visibilities, setVisibilities] = useState<AlertVisibility[]>(
    organization.verified_alert_visibilities,
  );
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [orgAdminEmail, setOrgAdminEmail] = useState("");
  const [invitingOrgAdmin, setInvitingOrgAdmin] = useState(false);
  const [inviteOrgAdminError, setInviteOrgAdminError] = useState<string | null>(null);
  const [togglingActive, setTogglingActive] = useState(false);
  const [toggleActiveError, setToggleActiveError] = useState<string | null>(null);

  const loadMembers = useCallback(async () => {
    try {
      setMembers(await listMembers(token, organization.organization_id));
    } catch (cause) {
      setMembersError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    }
  }, [token, organization.organization_id, t]);

  useEffect(() => {
    void loadMembers();
  }, [loadMembers]);

  async function handleSaveTrustGrants() {
    setSaving(true);
    setSaveError(null);
    try {
      const updated = await setTrustGrants(token, organization.organization_id, incidentTypes, visibilities);
      onUpdated(updated);
    } catch (cause) {
      setSaveError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setSaving(false);
    }
  }

  async function handleInviteOrgAdmin(event: FormEvent) {
    event.preventDefault();
    setInvitingOrgAdmin(true);
    setInviteOrgAdminError(null);
    try {
      await registerOrgAdmin(token, orgAdminEmail.trim(), organization.organization_id);
      setOrgAdminEmail("");
      await loadMembers();
    } catch (cause) {
      setInviteOrgAdminError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setInvitingOrgAdmin(false);
    }
  }

  async function handleToggleActive() {
    setTogglingActive(true);
    setToggleActiveError(null);
    try {
      const updated = organization.is_active
        ? await deactivateOrganization(token, organization.organization_id)
        : await reactivateOrganization(token, organization.organization_id);
      onUpdated(updated);
    } catch (cause) {
      setToggleActiveError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setTogglingActive(false);
    }
  }

  return (
    <div className="mt-3 rounded-xl border border-white/[0.06] bg-white/[0.02] p-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <span
          className={`rounded px-2 py-0.5 text-xs font-medium ${
            organization.is_active
              ? "bg-emerald-500/10 text-emerald-300"
              : "bg-red-500/10 text-red-300"
          }`}
        >
          {organization.is_active ? t.organizations.statusActive : t.organizations.statusInactive}
        </span>
        <button
          type="button"
          disabled={togglingActive}
          onClick={() => void handleToggleActive()}
          className="rounded-lg border border-white/[0.08] px-3 py-1.5 text-xs font-medium text-slate-300 transition-colors hover:bg-white/[0.05] disabled:cursor-not-allowed disabled:opacity-50"
        >
          {organization.is_active
            ? togglingActive
              ? t.organizations.deactivating
              : t.organizations.deactivate
            : togglingActive
              ? t.organizations.reactivating
              : t.organizations.reactivate}
        </button>
      </div>
      {toggleActiveError && (
        <p role="alert" className="mb-3 text-sm text-red-300">
          {toggleActiveError}
        </p>
      )}
      {!organization.is_active && (
        <p className="mb-3 text-xs text-amber-300">{t.organizations.inactiveNote}</p>
      )}

      <OrganizationProfileForm token={token} organization={organization} onUpdated={onUpdated} />

      <OrganizationContact token={token} organization={organization} />

      <h3 className="mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
        {t.organizations.trustGrantsHeading}
      </h3>
      <div className="mb-2">
        <p className="mb-1 text-xs text-slate-400">{t.organizations.verifiedIncidentTypes}</p>
        <div className="flex flex-wrap gap-3 text-sm text-slate-300">
          {INCIDENT_TYPES.map((value) => (
            <label key={value} className="flex items-center gap-1.5">
              <input
                type="checkbox"
                checked={incidentTypes.includes(value)}
                onChange={() => setIncidentTypes((current) => toggleValue(current, value))}
              />
              {value.replace(/_/g, " ")}
            </label>
          ))}
        </div>
      </div>
      <div className="mb-3">
        <p className="mb-1 text-xs text-slate-400">{t.organizations.verifiedVisibilities}</p>
        <div className="flex flex-wrap gap-3 text-sm text-slate-300">
          {ALERT_VISIBILITIES.map((value) => (
            <label key={value} className="flex items-center gap-1.5">
              <input
                type="checkbox"
                checked={visibilities.includes(value)}
                onChange={() => setVisibilities((current) => toggleValue(current, value))}
              />
              {value}
            </label>
          ))}
        </div>
      </div>
      {saveError && (
        <p role="alert" className="mb-2 text-sm text-red-300">
          {saveError}
        </p>
      )}
      <button
        type="button"
        disabled={saving}
        onClick={() => void handleSaveTrustGrants()}
        className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {saving ? t.organizations.saving : t.organizations.saveTrustGrants}
      </button>

      <form
        onSubmit={(event) => void handleInviteOrgAdmin(event)}
        className="mt-4 rounded-xl border border-white/[0.06] bg-white/[0.02] p-4"
      >
        <h3 className="mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
          {t.organizations.inviteOrgAdminHeading}
        </h3>
        <div className="flex flex-wrap items-end gap-2">
          <div className="flex-1">
            <label
              htmlFor={`org-admin-email-${organization.organization_id}`}
              className="mb-1 block text-xs text-slate-400"
            >
              {t.myOrganization.emailLabel}
            </label>
            <input
              id={`org-admin-email-${organization.organization_id}`}
              type="email"
              value={orgAdminEmail}
              placeholder={t.myOrganization.emailPlaceholder}
              onChange={(event) => setOrgAdminEmail(event.target.value)}
              className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
            />
          </div>
          <button
            type="submit"
            disabled={invitingOrgAdmin || !orgAdminEmail.trim() || !organization.is_active}
            className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {invitingOrgAdmin ? t.myOrganization.inviting : t.organizations.inviteOrgAdmin}
          </button>
        </div>
        {inviteOrgAdminError && (
          <p role="alert" className="mt-2 text-sm text-red-300">
            {inviteOrgAdminError}
          </p>
        )}
      </form>

      <h3 className="mt-4 mb-2 text-xs font-semibold tracking-wide text-slate-300 uppercase">
        {t.organizations.membersHeading}
      </h3>
      {membersError && (
        <p role="alert" className="text-sm text-red-300">
          {membersError}
        </p>
      )}
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

export function Organizations() {
  const { t } = useTranslation();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>/<RequirePlatformAdmin>.
  const token = session!.token;

  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const loadOrganizations = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setOrganizations(await listOrganizations(token));
    } catch (cause) {
      setLoadError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }, [token, t]);

  useEffect(() => {
    void loadOrganizations();
  }, [loadOrganizations]);

  function replaceOrganization(updated: Organization) {
    setOrganizations((current) =>
      current.map((org) => (org.organization_id === updated.organization_id ? updated : org)),
    );
  }

  return (
    <div>
      <h1 className="mb-6 text-2xl font-semibold text-white">{t.organizations.heading}</h1>

      <CreateOrganizationForm
        token={token}
        onCreated={(created) => setOrganizations((current) => [created, ...current])}
      />

      {loading && <p className="text-sm text-slate-400">{t.organizations.loading}</p>}
      {loadError && (
        <p role="alert" className="text-sm text-red-300">
          {loadError}
        </p>
      )}
      {!loading && organizations.length === 0 && !loadError && (
        <p className="text-sm text-slate-500">{t.organizations.noOrganizations}</p>
      )}

      <ul className="space-y-3">
        {organizations.map((organization) => {
          const expanded = expandedId === organization.organization_id;
          return (
            <li
              key={organization.organization_id}
              className="rounded-2xl border border-white/[0.08] bg-white/[0.03] p-4"
            >
              <div className="flex items-start justify-between gap-4">
                <div>
                  <div className="flex items-center gap-2">
                    <p className="font-medium text-white">{organization.name}</p>
                    {!organization.is_active && (
                      <span className="rounded bg-red-500/10 px-2 py-0.5 text-xs font-medium text-red-300">
                        {t.organizations.statusInactive}
                      </span>
                    )}
                  </div>
                  <TrustGrantSummary organization={organization} t={t} />
                </div>
                <button
                  type="button"
                  onClick={() => setExpandedId(expanded ? null : organization.organization_id)}
                  className="flex-shrink-0 rounded-lg border border-white/[0.08] px-3 py-1.5 text-xs text-slate-300 transition-colors hover:bg-white/[0.05] hover:text-white"
                >
                  {expanded ? t.organizations.hideDetails : t.organizations.viewDetails}
                </button>
              </div>
              {expanded && (
                <OrganizationDetail token={token} organization={organization} onUpdated={replaceOrganization} />
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

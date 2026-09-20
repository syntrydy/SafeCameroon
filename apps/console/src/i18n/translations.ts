import { BRAND_NAME } from "../config/brand";
import type { Locale } from "./locale";

export interface Translations {
  common: {
    unexpectedError: string;
    loading: string;
    cancel: string;
    remove: string;
    edit: string;
    save: string;
    saving: string;
  };
  brand: {
    consoleBadge: string;
  };
  layout: {
    reports: string;
    cases: string;
    alerts: string;
    subscriptions: string;
    organizations: string;
    myOrganization: string;
    reviewer: string;
    orgAdmin: string;
    platformAdmin: string;
    signOut: string;
    collapseSidebar: string;
    expandSidebar: string;
  };
  login: {
    heroHeadingPrefix: string;
    heroHeadingHighlight: string;
    heroSubtitle: string;
    fullAuditTrail: string;
    privacyFirst: string;
    organizationConsole: string;
    welcomeBack: string;
    signInSubtitle: string;
    secureNote: string;
    accessLimited: string;
  };
  footer: {
    tagline: string;
    copyright: (year: number) => string;
    citizenLink: string;
    switchToEnglish: string;
    switchToFrench: string;
  };
  reviewQueue: {
    heading: string;
    statusLabel: string;
    statusAll: string;
    statusReceived: string;
    statusUnderReview: string;
    statusLinkedToCase: string;
    statusClosed: string;
    caseCreated: string;
    linkedToCase: string;
    reviewStarted: string;
    viewCase: string;
    loading: string;
    noReports: string;
    colReport: string;
    colChannel: string;
    colStatus: string;
    colReceived: string;
    colContent: string;
    colActions: string;
  };
  reportRow: {
    showMore: string;
    showLess: string;
    incidentTypeLabel: (reportId: string) => string;
    incidentTypeMissingChild: string;
    incidentTypeOtherProtection: string;
    existingCaseIdLabel: (reportId: string) => string;
    existingCaseIdPlaceholder: string;
    startReview: string;
    createCase: string;
    linkToCase: string;
    reporterSuggested: string;
  };
  extraction: {
    toggle: string;
    extracting: string;
    extractButton: string;
    disclaimer: string;
    loading: string;
    none: string;
    noCandidateDetails: string;
    fieldDescription: string;
    fieldAge: string;
    fieldTime: string;
    fieldPlace: string;
    fieldIncidentCategory: string;
    fieldVehicleDetails: string;
    fieldContactRequest: string;
    applyButton: string;
  };
  caseDetail: {
    heading: (shortId: string) => string;
    version: (n: number) => string;
    caseMovedTo: (statusLabel: string) => string;
    loading: string;
    markStatus: (statusLabel: string) => string;
    linkedReports: string;
    noLinkedReports: string;
    history: string;
    statusReported: string;
    statusUnderReview: string;
    statusVerified: string;
    statusActive: string;
    statusResolved: string;
    statusCancelled: string;
    statusRejected: string;
  };
  linkedAttachments: {
    showAttachments: string;
    hideAttachments: string;
    loading: string;
    none: string;
    open: string;
    getDownloadLink: string;
    attachmentPreviewAlt: string;
    sizeBytes: (n: number) => string;
  };
  alertList: {
    heading: string;
    statusLabel: string;
    statusAll: string;
    statusActive: string;
    statusCancelled: string;
    loading: string;
    noAlerts: string;
    colAlert: string;
    colSeverity: string;
    colVisibility: string;
    colStatus: string;
    colTargetGeography: string;
  };
  caseList: {
    heading: string;
    statusLabel: string;
    statusAll: string;
    statusReported: string;
    statusUnderReview: string;
    statusVerified: string;
    statusActive: string;
    statusResolved: string;
    statusCancelled: string;
    statusRejected: string;
    loading: string;
    noCases: string;
    colCase: string;
    colIncidentType: string;
    colStatus: string;
    colReports: string;
    incidentTypeMissingChild: string;
    incidentTypeOtherProtection: string;
  };
  alertPreview: {
    heading: (shortId: string) => string;
    version: (n: number) => string;
    statusActive: string;
    statusCancelled: string;
    alertCancelled: string;
    loading: string;
    targetGeography: string;
    caseLabel: string;
    policyLabel: string;
    deliveriesLabel: string;
    viewDeliveries: string;
    safeProjection: string;
    noFields: string;
    cancelAlert: string;
    visibilityInternal: string;
    visibilityPartner: string;
    visibilityCommunity: string;
    visibilityPublic: string;
  };
  createAlertForm: {
    heading: string;
    severity: string;
    targetGeography: string;
    targetGeographyPlaceholder: string;
    targetGeographyRequired: string;
    noKnownAreasYet: string;
    removeArea: (area: string) => string;
    creating: string;
    createAlert: string;
    description: string;
    suggestionsApplied: string;
  };
  deliveryList: {
    heading: (shortId: string) => string;
    loading: string;
    none: string;
    colDelivery: string;
    colChannel: string;
    colTier: string;
    colStatus: string;
    colAttempts: string;
  };
  deliveryDetail: {
    heading: (shortId: string) => string;
    loading: string;
    channel: string;
    endpoint: string;
    tier: string;
    status: string;
    attempts: string;
    attemptHistory: string;
    noAttempts: string;
    colNumber: string;
    colOutcome: string;
    colDetail: string;
    providerMessageId: (id: string) => string;
    unknownFailure: string;
    retryable: string;
    permanent: string;
  };
  subscriptions: {
    heading: string;
    lookupHeading: string;
    consumerId: string;
    load: string;
    registerHeading: string;
    name: string;
    type: string;
    typeOrganization: string;
    typeCitizen: string;
    createConsumer: string;
    switchConsumer: string;
    subscriptionsHeading: string;
    loading: string;
    deliveryPreferenceHeading: string;
    noDeliveryPreference: string;
    addSubscription: string;
    creating: string;
    createSubscription: string;
    allSubscriptionsHeading: string;
    noSubscriptionsAtAll: string;
    consumerLabel: string;
    viewConsumer: string;
    loadMore: string;
    colSubscriber: string;
    colRules: string;
    colVersion: string;
    colCreated: string;
    colActions: string;
  };
  subscriptionCard: {
    edit: string;
    saving: string;
    saveChanges: string;
    cancel: string;
  };
  ruleEditor: {
    ruleIncidentType: string;
    ruleSeverity: string;
    ruleEventType: string;
    ruleGeography: string;
    remove: string;
    addRule: string;
    geographyPlaceholder: string;
    addArea: string;
    removeArea: (area: string) => string;
    comparisonGreaterThan: string;
    comparisonGreaterThanOrEqual: string;
    comparisonEqual: string;
    comparisonLessThanOrEqual: string;
    comparisonLessThan: string;
  };
  describeRule: {
    incidentType: (values: string) => string;
    noneSelected: string;
    severity: (operator: string, value: string) => string;
    eventType: (values: string) => string;
    geography: (area: string) => string;
    none: string;
  };
  deliveryPreferenceForm: {
    strategy: string;
    strategyAll: string;
    strategyPrimaryFallback: string;
    strategyPriorityList: string;
    channelLabel: (n: number) => string;
    addressLabel: (n: number) => string;
    addressPlaceholder: string;
    addChannel: string;
    saving: string;
    saveDeliveryPreference: string;
  };
  organizations: {
    heading: string;
    createHeading: string;
    name: string;
    descriptionLabel: string;
    locationLabel: string;
    creating: string;
    create: string;
    loading: string;
    noOrganizations: string;
    verifiedIncidentTypes: string;
    verifiedVisibilities: string;
    none: string;
    viewDetails: string;
    hideDetails: string;
    membersHeading: string;
    noMembers: string;
    trustGrantsHeading: string;
    saveTrustGrants: string;
    saving: string;
    inviteOrgAdminHeading: string;
    inviteOrgAdmin: string;
    contactLabel: string;
    profileSaved: string;
    profileHeading: string;
    saveProfile: string;
    savingProfile: string;
    noDescription: string;
    noLocation: string;
    statusActive: string;
    statusInactive: string;
    deactivate: string;
    deactivating: string;
    reactivate: string;
    reactivating: string;
    inactiveNote: string;
  };
  myOrganization: {
    heading: string;
    loading: string;
    noOrganization: string;
    goToOrganizations: string;
    inviteHeading: string;
    emailLabel: string;
    emailPlaceholder: string;
    invite: string;
    inviting: string;
  };
}

export const translations: Record<Locale, Translations> = {
  en: {
    common: {
      unexpectedError: "An unexpected error occurred.",
      loading: "Loading...",
      cancel: "Cancel",
      remove: "Remove",
      edit: "Edit",
      save: "Save",
      saving: "Saving...",
    },
    brand: {
      consoleBadge: "Portal",
    },
    layout: {
      reports: "Reports",
      cases: "Cases",
      alerts: "Alerts",
      subscriptions: "Subscriptions",
      organizations: "Organizations",
      myOrganization: "My organization",
      reviewer: "Reviewer",
      orgAdmin: "Org admin",
      platformAdmin: "Admin",
      signOut: "Sign out",
      collapseSidebar: "Collapse sidebar",
      expandSidebar: "Expand sidebar",
    },
    login: {
      heroHeadingPrefix: "Coordinated response for",
      heroHeadingHighlight: "every reported incident",
      heroSubtitle:
        "Verified reviewers triage reports, confirm cases, and issue alerts through a single accountable portal—built for privacy, speed, and oversight.",
      fullAuditTrail: "Full audit trail",
      privacyFirst: "Privacy-first architecture",
      organizationConsole: "Organization portal",
      welcomeBack: "Welcome back",
      signInSubtitle: "Sign in with your registered Google account to access the portal.",
      secureNote: "Secured with OAuth 2.0 · Your credentials are never stored",
      accessLimited: "Access is limited to reviewers added by an administrator.",
    },
    footer: {
      tagline: "Privacy-first civic-protection platform",
      copyright: (year) => `© ${year} ${BRAND_NAME}`,
      citizenLink: "Report an incident anonymously",
      switchToEnglish: "Switch to English",
      switchToFrench: "Switch to French",
    },
    reviewQueue: {
      heading: "Review queue",
      statusLabel: "Status:",
      statusAll: "All",
      statusReceived: "Received",
      statusUnderReview: "Under review",
      statusLinkedToCase: "Linked to case",
      statusClosed: "Closed",
      caseCreated: "Case created.",
      linkedToCase: "Linked to case.",
      reviewStarted: "Review started.",
      viewCase: "View case",
      loading: "Loading reports...",
      noReports: "No reports match this filter.",
      colReport: "Report",
      colChannel: "Channel",
      colStatus: "Status",
      colReceived: "Received",
      colContent: "Content",
      colActions: "Actions",
    },
    reportRow: {
      showMore: "Show more",
      showLess: "Show less",
      incidentTypeLabel: (reportId) => `Incident type for report ${reportId}`,
      incidentTypeMissingChild: "Missing child",
      incidentTypeOtherProtection: "Other protection incident",
      existingCaseIdLabel: (reportId) => `Existing case id for report ${reportId}`,
      existingCaseIdPlaceholder: "Existing case id",
      startReview: "Start review",
      createCase: "Create case",
      linkToCase: "Link to case",
      reporterSuggested: "Reporter suggested:",
    },
    extraction: {
      toggle: "AI extraction",
      extracting: "Extracting...",
      extractButton: "Extract candidate info",
      disclaimer:
        "Unverified suggestion, not a fact -- confirm anything relevant yourself before acting on it.",
      loading: "Loading...",
      none: "No extraction requested yet.",
      noCandidateDetails: "No candidate details found.",
      fieldDescription: "Description",
      fieldAge: "Age",
      fieldTime: "Time",
      fieldPlace: "Place",
      fieldIncidentCategory: "Suggested category",
      fieldVehicleDetails: "Vehicle",
      fieldContactRequest: "Contact request",
      applyButton: "Apply to alert form",
    },
    caseDetail: {
      heading: (shortId) => `Case ${shortId}`,
      version: (n) => `version ${n}`,
      caseMovedTo: (statusLabel) => `Case moved to ${statusLabel}.`,
      loading: "Loading case...",
      markStatus: (statusLabel) => `Mark ${statusLabel}`,
      linkedReports: "Linked reports",
      noLinkedReports: "No linked reports.",
      history: "History",
      statusReported: "Reported",
      statusUnderReview: "Under review",
      statusVerified: "Verified",
      statusActive: "Active",
      statusResolved: "Resolved",
      statusCancelled: "Cancelled",
      statusRejected: "Rejected",
    },
    linkedAttachments: {
      showAttachments: "Show attachments",
      hideAttachments: "Hide attachments",
      loading: "Loading attachments...",
      none: "No attachments.",
      open: "Open",
      getDownloadLink: "Get download link",
      attachmentPreviewAlt: "Attachment preview",
      sizeBytes: (n) => `${n} bytes`,
    },
    alertList: {
      heading: "Alerts",
      statusLabel: "Status:",
      statusAll: "All",
      statusActive: "Active",
      statusCancelled: "Cancelled",
      loading: "Loading alerts...",
      noAlerts: "No alerts match this filter.",
      colAlert: "Alert",
      colSeverity: "Severity",
      colVisibility: "Visibility",
      colStatus: "Status",
      colTargetGeography: "Target geography",
    },
    caseList: {
      heading: "Cases",
      statusLabel: "Status:",
      statusAll: "All",
      statusReported: "Reported",
      statusUnderReview: "Under review",
      statusVerified: "Verified",
      statusActive: "Active",
      statusResolved: "Resolved",
      statusCancelled: "Cancelled",
      statusRejected: "Rejected",
      loading: "Loading cases...",
      noCases: "No cases match this filter.",
      colCase: "Case",
      colIncidentType: "Incident type",
      colStatus: "Status",
      colReports: "Reports",
      incidentTypeMissingChild: "Missing child",
      incidentTypeOtherProtection: "Other protection incident",
    },
    alertPreview: {
      heading: (shortId) => `Alert ${shortId}`,
      version: (n) => `version ${n}`,
      statusActive: "Active",
      statusCancelled: "Cancelled",
      alertCancelled: "Alert cancelled.",
      loading: "Loading alert...",
      targetGeography: "Target geography",
      caseLabel: "Case",
      policyLabel: "Policy",
      deliveriesLabel: "Deliveries",
      viewDeliveries: "View deliveries",
      safeProjection: "Safe projection",
      noFields: "No fields were included.",
      cancelAlert: "Cancel alert",
      visibilityInternal: "Internal",
      visibilityPartner: "Partner",
      visibilityCommunity: "Community",
      visibilityPublic: "Public",
    },
    createAlertForm: {
      heading: "Create community alert",
      severity: "Severity",
      targetGeography: "Target geography",
      targetGeographyPlaceholder: "Type to search subscribed areas...",
      targetGeographyRequired: "Select at least one area -- this is who gets notified.",
      noKnownAreasYet: "No one has subscribed to any area yet; type a new one.",
      removeArea: (area) => `Remove ${area}`,
      creating: "Creating...",
      createAlert: "Create alert",
      description: "Alert description",
      suggestionsApplied:
        "Filled in from the report's AI suggestion -- review every value before sending.",
    },
    deliveryList: {
      heading: (shortId) => `Deliveries for alert ${shortId}`,
      loading: "Loading deliveries...",
      none: "No deliveries have been planned for this alert.",
      colDelivery: "Delivery",
      colChannel: "Channel",
      colTier: "Tier",
      colStatus: "Status",
      colAttempts: "Attempts",
    },
    deliveryDetail: {
      heading: (shortId) => `Delivery ${shortId}`,
      loading: "Loading delivery...",
      channel: "Channel",
      endpoint: "Endpoint",
      tier: "Tier",
      status: "Status",
      attempts: "Attempts",
      attemptHistory: "Attempt history",
      noAttempts: "No attempts have been made yet.",
      colNumber: "#",
      colOutcome: "Outcome",
      colDetail: "Detail",
      providerMessageId: (id) => `Provider message id: ${id}`,
      unknownFailure: "Unknown failure",
      retryable: "retryable",
      permanent: "permanent",
    },
    subscriptions: {
      heading: "Subscriptions",
      lookupHeading: "Look up a subscriber",
      consumerId: "Subscriber id",
      load: "Load",
      registerHeading: "Register a new subscriber",
      name: "Name",
      type: "Type",
      typeOrganization: "Organization",
      typeCitizen: "Citizen",
      createConsumer: "Create subscriber",
      switchConsumer: "Switch subscriber",
      subscriptionsHeading: "Subscriptions",
      loading: "Loading...",
      deliveryPreferenceHeading: "Delivery preference",
      noDeliveryPreference: "No delivery preference is set yet.",
      addSubscription: "Add subscription",
      creating: "Creating...",
      createSubscription: "Create subscription",
      allSubscriptionsHeading: "All subscribers",
      noSubscriptionsAtAll: "No subscribers exist yet.",
      consumerLabel: "Subscriber",
      viewConsumer: "View subscriber",
      loadMore: "Load more",
      colSubscriber: "Subscriber",
      colRules: "Rules",
      colVersion: "Version",
      colCreated: "Created",
      colActions: "Actions",
    },
    subscriptionCard: {
      edit: "Edit",
      saving: "Saving...",
      saveChanges: "Save changes",
      cancel: "Cancel",
    },
    ruleEditor: {
      ruleIncidentType: "Incident type",
      ruleSeverity: "Severity",
      ruleEventType: "Event type",
      ruleGeography: "Geography",
      remove: "Remove",
      addRule: "Add rule",
      geographyPlaceholder: "e.g. Douala",
      addArea: "Add",
      removeArea: (area) => `Remove ${area}`,
      comparisonGreaterThan: "> greater than",
      comparisonGreaterThanOrEqual: ">= greater than or equal",
      comparisonEqual: "== equal",
      comparisonLessThanOrEqual: "<= less than or equal",
      comparisonLessThan: "< less than",
    },
    describeRule: {
      incidentType: (values) => `Incident type: ${values}`,
      noneSelected: "(none selected)",
      severity: (operator, value) => `Severity ${operator} ${value}`,
      eventType: (values) => `Event type: ${values}`,
      geography: (area) => `Geography: ${area}`,
      none: "(none)",
    },
    deliveryPreferenceForm: {
      strategy: "Strategy",
      strategyAll: "All channels",
      strategyPrimaryFallback: "Primary, then fallback",
      strategyPriorityList: "Priority order",
      channelLabel: (n) => `Channel ${n}`,
      addressLabel: (n) => `Address ${n}`,
      addressPlaceholder: "e.g. +237600000000",
      addChannel: "Add channel",
      saving: "Saving...",
      saveDeliveryPreference: "Save delivery preference",
    },
    organizations: {
      heading: "Organizations",
      createHeading: "Register a new organization",
      name: "Name",
      descriptionLabel: "Description (optional)",
      locationLabel: "Location (optional)",
      creating: "Creating...",
      create: "Create organization",
      loading: "Loading organizations...",
      noOrganizations: "No organizations registered yet.",
      verifiedIncidentTypes: "Verified incident types",
      verifiedVisibilities: "Verified alert visibilities",
      none: "None",
      viewDetails: "View details",
      hideDetails: "Hide details",
      membersHeading: "Members",
      noMembers: "No members yet.",
      trustGrantsHeading: "Trust grants",
      saveTrustGrants: "Save trust grants",
      saving: "Saving...",
      inviteOrgAdminHeading: "Invite an org admin",
      inviteOrgAdmin: "Invite org admin",
      contactLabel: "Contact details",
      profileSaved: "Organization profile saved.",
      profileHeading: "Profile",
      saveProfile: "Save profile",
      savingProfile: "Saving...",
      noDescription: "No description yet.",
      noLocation: "No location set.",
      statusActive: "Active",
      statusInactive: "Deactivated",
      deactivate: "Deactivate",
      deactivating: "Deactivating...",
      reactivate: "Reactivate",
      reactivating: "Reactivating...",
      inactiveNote:
        "This organization is deactivated. Its members can no longer verify cases, issue alerts, or manage notification settings, and it cannot register new members until reactivated.",
    },
    myOrganization: {
      heading: "My organization",
      loading: "Loading organization...",
      noOrganization: "You are not a member of any organization.",
      goToOrganizations: "Go to Organizations",
      inviteHeading: "Invite a reviewer",
      emailLabel: "Email",
      emailPlaceholder: "name@example.org",
      invite: "Invite",
      inviting: "Inviting...",
    },
  },
  fr: {
    common: {
      unexpectedError: "Une erreur inattendue s'est produite.",
      loading: "Chargement...",
      cancel: "Annuler",
      remove: "Retirer",
      edit: "Modifier",
      save: "Enregistrer",
      saving: "Enregistrement...",
    },
    brand: {
      consoleBadge: "Portail",
    },
    layout: {
      reports: "Signalements",
      cases: "Dossiers",
      alerts: "Alertes",
      subscriptions: "Abonnements",
      organizations: "Organisations",
      myOrganization: "Mon organisation",
      reviewer: "Examinateur",
      orgAdmin: "Administrateur d'organisation",
      platformAdmin: "Administrateur",
      signOut: "Se déconnecter",
      collapseSidebar: "Réduire la barre latérale",
      expandSidebar: "Agrandir la barre latérale",
    },
    login: {
      heroHeadingPrefix: "Une réponse coordonnée pour",
      heroHeadingHighlight: "chaque incident signalé",
      heroSubtitle:
        "Des examinateurs vérifiés trient les signalements, confirment les dossiers et publient des alertes via un portail unique et responsable—conçu pour la confidentialité, la rapidité et la supervision.",
      fullAuditTrail: "Journal d'audit complet",
      privacyFirst: "Architecture axée sur la confidentialité",
      organizationConsole: "Portail de l'organisation",
      welcomeBack: "Bon retour",
      signInSubtitle:
        "Connectez-vous avec votre compte Google enregistré pour accéder au portail.",
      secureNote:
        "Sécurisé avec OAuth 2.0 · Vos identifiants ne sont jamais stockés",
      accessLimited: "L'accès est réservé aux examinateurs ajoutés par un administrateur.",
    },
    footer: {
      tagline: "Plateforme de protection civile axée sur la confidentialité",
      copyright: (year) => `© ${year} ${BRAND_NAME}`,
      citizenLink: "Signaler un incident anonymement",
      switchToEnglish: "Passer à l'anglais",
      switchToFrench: "Passer au français",
    },
    reviewQueue: {
      heading: "File d'examen",
      statusLabel: "Statut :",
      statusAll: "Tous",
      statusReceived: "Reçu",
      statusUnderReview: "En cours d'examen",
      statusLinkedToCase: "Lié à un dossier",
      statusClosed: "Clôturé",
      caseCreated: "Dossier créé.",
      linkedToCase: "Lié au dossier.",
      reviewStarted: "Examen commencé.",
      viewCase: "Voir le dossier",
      loading: "Chargement des signalements...",
      noReports: "Aucun signalement ne correspond à ce filtre.",
      colReport: "Signalement",
      colChannel: "Canal",
      colStatus: "Statut",
      colReceived: "Reçu le",
      colContent: "Contenu",
      colActions: "Actions",
    },
    reportRow: {
      showMore: "Afficher plus",
      showLess: "Afficher moins",
      incidentTypeLabel: (reportId) => `Type d'incident pour le signalement ${reportId}`,
      incidentTypeMissingChild: "Enfant disparu",
      incidentTypeOtherProtection: "Autre incident de protection",
      existingCaseIdLabel: (reportId) => `Identifiant de dossier existant pour le signalement ${reportId}`,
      existingCaseIdPlaceholder: "Identifiant de dossier existant",
      startReview: "Commencer l'examen",
      createCase: "Créer un dossier",
      linkToCase: "Lier à un dossier",
      reporterSuggested: "Suggéré par le rapporteur :",
    },
    extraction: {
      toggle: "Extraction IA",
      extracting: "Extraction en cours...",
      extractButton: "Extraire les informations candidates",
      disclaimer:
        "Suggestion non vérifiée, pas un fait -- confirmez vous-même tout élément pertinent avant d'agir.",
      loading: "Chargement...",
      none: "Aucune extraction demandée pour l'instant.",
      noCandidateDetails: "Aucun détail candidat trouvé.",
      fieldDescription: "Description",
      fieldAge: "Âge",
      fieldTime: "Heure",
      fieldPlace: "Lieu",
      fieldIncidentCategory: "Catégorie suggérée",
      fieldVehicleDetails: "Véhicule",
      fieldContactRequest: "Demande de contact",
      applyButton: "Appliquer au formulaire d'alerte",
    },
    caseDetail: {
      heading: (shortId) => `Dossier ${shortId}`,
      version: (n) => `version ${n}`,
      caseMovedTo: (statusLabel) => `Dossier passé à ${statusLabel}.`,
      loading: "Chargement du dossier...",
      markStatus: (statusLabel) => `Marquer ${statusLabel}`,
      linkedReports: "Signalements liés",
      noLinkedReports: "Aucun signalement lié.",
      history: "Historique",
      statusReported: "Signalé",
      statusUnderReview: "En cours d'examen",
      statusVerified: "Vérifié",
      statusActive: "Actif",
      statusResolved: "Résolu",
      statusCancelled: "Annulé",
      statusRejected: "Rejeté",
    },
    linkedAttachments: {
      showAttachments: "Afficher les pièces jointes",
      hideAttachments: "Masquer les pièces jointes",
      loading: "Chargement des pièces jointes...",
      none: "Aucune pièce jointe.",
      open: "Ouvrir",
      getDownloadLink: "Obtenir le lien de téléchargement",
      attachmentPreviewAlt: "Aperçu de la pièce jointe",
      sizeBytes: (n) => `${n} octets`,
    },
    alertList: {
      heading: "Alertes",
      statusLabel: "Statut :",
      statusAll: "Toutes",
      statusActive: "Active",
      statusCancelled: "Annulée",
      loading: "Chargement des alertes...",
      noAlerts: "Aucune alerte ne correspond à ce filtre.",
      colAlert: "Alerte",
      colSeverity: "Gravité",
      colVisibility: "Visibilité",
      colStatus: "Statut",
      colTargetGeography: "Zone ciblée",
    },
    caseList: {
      heading: "Dossiers",
      statusLabel: "Statut :",
      statusAll: "Tous",
      statusReported: "Signalé",
      statusUnderReview: "En cours d'examen",
      statusVerified: "Vérifié",
      statusActive: "Actif",
      statusResolved: "Résolu",
      statusCancelled: "Annulé",
      statusRejected: "Rejeté",
      loading: "Chargement des dossiers...",
      noCases: "Aucun dossier ne correspond à ce filtre.",
      colCase: "Dossier",
      colIncidentType: "Type d'incident",
      colStatus: "Statut",
      colReports: "Signalements",
      incidentTypeMissingChild: "Enfant disparu",
      incidentTypeOtherProtection: "Autre incident de protection",
    },
    alertPreview: {
      heading: (shortId) => `Alerte ${shortId}`,
      version: (n) => `version ${n}`,
      statusActive: "Active",
      statusCancelled: "Annulée",
      alertCancelled: "Alerte annulée.",
      loading: "Chargement de l'alerte...",
      targetGeography: "Zone ciblée",
      caseLabel: "Dossier",
      policyLabel: "Politique",
      deliveriesLabel: "Envois",
      viewDeliveries: "Voir les envois",
      safeProjection: "Projection sécurisée",
      noFields: "Aucun champ n'a été inclus.",
      cancelAlert: "Annuler l'alerte",
      visibilityInternal: "Interne",
      visibilityPartner: "Partenaire",
      visibilityCommunity: "Communauté",
      visibilityPublic: "Public",
    },
    createAlertForm: {
      heading: "Créer une alerte communautaire",
      severity: "Gravité",
      targetGeography: "Zone ciblée",
      targetGeographyPlaceholder: "Tapez pour rechercher les zones abonnées...",
      targetGeographyRequired: "Sélectionnez au moins une zone -- c'est qui sera notifié.",
      noKnownAreasYet: "Personne n'est encore abonné à une zone ; tapez-en une nouvelle.",
      removeArea: (area) => `Retirer ${area}`,
      creating: "Création...",
      createAlert: "Créer l'alerte",
      description: "Description de l'alerte",
      suggestionsApplied:
        "Rempli à partir de la suggestion IA du signalement -- vérifiez chaque valeur avant l'envoi.",
    },
    deliveryList: {
      heading: (shortId) => `Envois pour l'alerte ${shortId}`,
      loading: "Chargement des envois...",
      none: "Aucun envoi n'a été planifié pour cette alerte.",
      colDelivery: "Envoi",
      colChannel: "Canal",
      colTier: "Niveau",
      colStatus: "Statut",
      colAttempts: "Tentatives",
    },
    deliveryDetail: {
      heading: (shortId) => `Envoi ${shortId}`,
      loading: "Chargement de l'envoi...",
      channel: "Canal",
      endpoint: "Destination",
      tier: "Niveau",
      status: "Statut",
      attempts: "Tentatives",
      attemptHistory: "Historique des tentatives",
      noAttempts: "Aucune tentative n'a encore été effectuée.",
      colNumber: "#",
      colOutcome: "Résultat",
      colDetail: "Détail",
      providerMessageId: (id) => `Identifiant du message fournisseur : ${id}`,
      unknownFailure: "Échec inconnu",
      retryable: "réessayable",
      permanent: "permanent",
    },
    subscriptions: {
      heading: "Abonnements",
      lookupHeading: "Rechercher un abonné",
      consumerId: "Identifiant de l'abonné",
      load: "Charger",
      registerHeading: "Enregistrer un nouvel abonné",
      name: "Nom",
      type: "Type",
      typeOrganization: "Organisation",
      typeCitizen: "Citoyen",
      createConsumer: "Créer l'abonné",
      switchConsumer: "Changer d'abonné",
      subscriptionsHeading: "Abonnements",
      loading: "Chargement...",
      deliveryPreferenceHeading: "Préférence de livraison",
      noDeliveryPreference: "Aucune préférence de livraison n'est encore définie.",
      addSubscription: "Ajouter un abonnement",
      creating: "Création...",
      createSubscription: "Créer l'abonnement",
      allSubscriptionsHeading: "Tous les abonnés",
      noSubscriptionsAtAll: "Aucun abonné n'existe pour l'instant.",
      consumerLabel: "Abonné",
      viewConsumer: "Voir l'abonné",
      loadMore: "Charger plus",
      colSubscriber: "Abonné",
      colRules: "Règles",
      colVersion: "Version",
      colCreated: "Créé le",
      colActions: "Actions",
    },
    subscriptionCard: {
      edit: "Modifier",
      saving: "Enregistrement...",
      saveChanges: "Enregistrer les modifications",
      cancel: "Annuler",
    },
    ruleEditor: {
      ruleIncidentType: "Type d'incident",
      ruleSeverity: "Gravité",
      ruleEventType: "Type d'événement",
      ruleGeography: "Zone géographique",
      remove: "Retirer",
      addRule: "Ajouter une règle",
      geographyPlaceholder: "ex. Douala",
      addArea: "Ajouter",
      removeArea: (area) => `Retirer ${area}`,
      comparisonGreaterThan: "> supérieur à",
      comparisonGreaterThanOrEqual: ">= supérieur ou égal à",
      comparisonEqual: "== égal à",
      comparisonLessThanOrEqual: "<= inférieur ou égal à",
      comparisonLessThan: "< inférieur à",
    },
    describeRule: {
      incidentType: (values) => `Type d'incident : ${values}`,
      noneSelected: "(aucun sélectionné)",
      severity: (operator, value) => `Gravité ${operator} ${value}`,
      eventType: (values) => `Type d'événement : ${values}`,
      geography: (area) => `Zone géographique : ${area}`,
      none: "(aucune)",
    },
    deliveryPreferenceForm: {
      strategy: "Stratégie",
      strategyAll: "Tous les canaux",
      strategyPrimaryFallback: "Principal, puis secours",
      strategyPriorityList: "Ordre de priorité",
      channelLabel: (n) => `Canal ${n}`,
      addressLabel: (n) => `Adresse ${n}`,
      addressPlaceholder: "ex. +237600000000",
      addChannel: "Ajouter un canal",
      saving: "Enregistrement...",
      saveDeliveryPreference: "Enregistrer la préférence de livraison",
    },
    organizations: {
      heading: "Organisations",
      createHeading: "Enregistrer une nouvelle organisation",
      name: "Nom",
      descriptionLabel: "Description (facultatif)",
      locationLabel: "Lieu (facultatif)",
      creating: "Création...",
      create: "Créer l'organisation",
      loading: "Chargement des organisations...",
      noOrganizations: "Aucune organisation enregistrée pour l'instant.",
      verifiedIncidentTypes: "Types d'incidents vérifiés",
      verifiedVisibilities: "Visibilités d'alerte vérifiées",
      none: "Aucun",
      viewDetails: "Voir les détails",
      hideDetails: "Masquer les détails",
      membersHeading: "Membres",
      noMembers: "Aucun membre pour l'instant.",
      trustGrantsHeading: "Habilitations de confiance",
      saveTrustGrants: "Enregistrer les habilitations",
      saving: "Enregistrement...",
      inviteOrgAdminHeading: "Inviter un administrateur d'organisation",
      inviteOrgAdmin: "Inviter l'administrateur",
      contactLabel: "Coordonnées",
      profileSaved: "Profil de l’organisation enregistré.",
      profileHeading: "Profil",
      saveProfile: "Enregistrer le profil",
      savingProfile: "Enregistrement...",
      noDescription: "Aucune description pour l'instant.",
      noLocation: "Aucun lieu défini.",
      statusActive: "Active",
      statusInactive: "Désactivée",
      deactivate: "Désactiver",
      deactivating: "Désactivation...",
      reactivate: "Réactiver",
      reactivating: "Réactivation...",
      inactiveNote:
        "Cette organisation est désactivée. Ses membres ne peuvent plus vérifier de dossiers, émettre d'alertes ni gérer les paramètres de notification, et elle ne peut plus inscrire de nouveaux membres jusqu'à sa réactivation.",
    },
    myOrganization: {
      heading: "Mon organisation",
      loading: "Chargement de l'organisation...",
      noOrganization: "Vous n'êtes membre d'aucune organisation.",
      goToOrganizations: "Aller à Organisations",
      inviteHeading: "Inviter un examinateur",
      emailLabel: "E-mail",
      emailPlaceholder: "nom@exemple.org",
      invite: "Inviter",
      inviting: "Invitation...",
    },
  },
};

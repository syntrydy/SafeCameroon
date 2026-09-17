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
    name: string;
    consoleBadge: string;
  };
  layout: {
    reports: string;
    alerts: string;
    subscriptions: string;
    reviewer: string;
    signOut: string;
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
    viewCase: string;
    loading: string;
    noReports: string;
    colReport: string;
    colChannel: string;
    colStatus: string;
    colReceived: string;
    colContentActions: string;
  };
  reportRow: {
    showMore: string;
    showLess: string;
    incidentTypeLabel: (reportId: string) => string;
    incidentTypeMissingChild: string;
    incidentTypeOtherProtection: string;
    existingCaseIdLabel: (reportId: string) => string;
    existingCaseIdPlaceholder: string;
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
    creating: string;
    createAlert: string;
    fieldIncidentCategory: string;
    fieldApproximateAge: string;
    fieldLastSeenArea: string;
    fieldTimeWindow: string;
    fieldSafeDescription: string;
    fieldOfficialContact: string;
    fieldCaseReference: string;
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
      name: "SafeCameroon",
      consoleBadge: "Console",
    },
    layout: {
      reports: "Reports",
      alerts: "Alerts",
      subscriptions: "Subscriptions",
      reviewer: "Reviewer",
      signOut: "Sign out",
    },
    login: {
      heroHeadingPrefix: "Coordinated response for",
      heroHeadingHighlight: "every reported incident",
      heroSubtitle:
        "Verified reviewers triage reports, confirm cases, and issue alerts through a single accountable console—built for privacy, speed, and oversight.",
      fullAuditTrail: "Full audit trail",
      privacyFirst: "Privacy-first architecture",
      organizationConsole: "Organization console",
      welcomeBack: "Welcome back",
      signInSubtitle: "Sign in with your registered Google account to access the console.",
      secureNote: "Secured with OAuth 2.0 · Your credentials are never stored",
      accessLimited: "Access is limited to reviewers added by an administrator.",
    },
    footer: {
      tagline: "Privacy-first civic-protection platform",
      copyright: (year) => `© ${year} SafeCameroon`,
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
      viewCase: "View case",
      loading: "Loading reports...",
      noReports: "No reports match this filter.",
      colReport: "Report",
      colChannel: "Channel",
      colStatus: "Status",
      colReceived: "Received",
      colContentActions: "Content & actions",
    },
    reportRow: {
      showMore: "Show more",
      showLess: "Show less",
      incidentTypeLabel: (reportId) => `Incident type for report ${reportId}`,
      incidentTypeMissingChild: "Missing child",
      incidentTypeOtherProtection: "Other protection incident",
      existingCaseIdLabel: (reportId) => `Existing case id for report ${reportId}`,
      existingCaseIdPlaceholder: "Existing case id",
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
      targetGeographyPlaceholder: "e.g. Douala, Bonamoussadi",
      creating: "Creating...",
      createAlert: "Create alert",
      fieldIncidentCategory: "Incident category",
      fieldApproximateAge: "Approximate age",
      fieldLastSeenArea: "Last seen (general area)",
      fieldTimeWindow: "Time window",
      fieldSafeDescription: "Safe description",
      fieldOfficialContact: "Official contact",
      fieldCaseReference: "Case reference",
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
      lookupHeading: "Look up a consumer",
      consumerId: "Consumer id",
      load: "Load",
      registerHeading: "Register a new consumer",
      name: "Name",
      type: "Type",
      typeOrganization: "Organization",
      typeCitizen: "Citizen",
      createConsumer: "Create consumer",
      switchConsumer: "Switch consumer",
      subscriptionsHeading: "Subscriptions",
      loading: "Loading...",
      deliveryPreferenceHeading: "Delivery preference",
      noDeliveryPreference: "No delivery preference is set yet.",
      addSubscription: "Add subscription",
      creating: "Creating...",
      createSubscription: "Create subscription",
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
      name: "SafeCameroon",
      consoleBadge: "Console",
    },
    layout: {
      reports: "Signalements",
      alerts: "Alertes",
      subscriptions: "Abonnements",
      reviewer: "Examinateur",
      signOut: "Se déconnecter",
    },
    login: {
      heroHeadingPrefix: "Une réponse coordonnée pour",
      heroHeadingHighlight: "chaque incident signalé",
      heroSubtitle:
        "Des examinateurs vérifiés trient les signalements, confirment les dossiers et publient des alertes via une console unique et responsable—conçue pour la confidentialité, la rapidité et la supervision.",
      fullAuditTrail: "Journal d'audit complet",
      privacyFirst: "Architecture axée sur la confidentialité",
      organizationConsole: "Console de l'organisation",
      welcomeBack: "Bon retour",
      signInSubtitle:
        "Connectez-vous avec votre compte Google enregistré pour accéder à la console.",
      secureNote:
        "Sécurisé avec OAuth 2.0 · Vos identifiants ne sont jamais stockés",
      accessLimited: "L'accès est réservé aux examinateurs ajoutés par un administrateur.",
    },
    footer: {
      tagline: "Plateforme de protection civile axée sur la confidentialité",
      copyright: (year) => `© ${year} SafeCameroon`,
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
      viewCase: "Voir le dossier",
      loading: "Chargement des signalements...",
      noReports: "Aucun signalement ne correspond à ce filtre.",
      colReport: "Signalement",
      colChannel: "Canal",
      colStatus: "Statut",
      colReceived: "Reçu le",
      colContentActions: "Contenu et actions",
    },
    reportRow: {
      showMore: "Afficher plus",
      showLess: "Afficher moins",
      incidentTypeLabel: (reportId) => `Type d'incident pour le signalement ${reportId}`,
      incidentTypeMissingChild: "Enfant disparu",
      incidentTypeOtherProtection: "Autre incident de protection",
      existingCaseIdLabel: (reportId) => `Identifiant de dossier existant pour le signalement ${reportId}`,
      existingCaseIdPlaceholder: "Identifiant de dossier existant",
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
      targetGeographyPlaceholder: "ex. Douala, Bonamoussadi",
      creating: "Création...",
      createAlert: "Créer l'alerte",
      fieldIncidentCategory: "Catégorie d'incident",
      fieldApproximateAge: "Âge approximatif",
      fieldLastSeenArea: "Dernière localisation (zone générale)",
      fieldTimeWindow: "Période",
      fieldSafeDescription: "Description sécurisée",
      fieldOfficialContact: "Contact officiel",
      fieldCaseReference: "Référence du dossier",
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
      lookupHeading: "Rechercher un consommateur",
      consumerId: "Identifiant du consommateur",
      load: "Charger",
      registerHeading: "Enregistrer un nouveau consommateur",
      name: "Nom",
      type: "Type",
      typeOrganization: "Organisation",
      typeCitizen: "Citoyen",
      createConsumer: "Créer le consommateur",
      switchConsumer: "Changer de consommateur",
      subscriptionsHeading: "Abonnements",
      loading: "Chargement...",
      deliveryPreferenceHeading: "Préférence de livraison",
      noDeliveryPreference: "Aucune préférence de livraison n'est encore définie.",
      addSubscription: "Ajouter un abonnement",
      creating: "Création...",
      createSubscription: "Créer l'abonnement",
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
  },
};

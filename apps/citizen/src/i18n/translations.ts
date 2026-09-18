import { BRAND_NAME } from "../config/brand";
import type { Locale } from "./locale";

export interface Translations {
  brand: {
    citizenBadge: string;
  };
  tabs: {
    report: string;
    alerts: string;
  };
  reportForm: {
    heading: string;
    subtitle: string;
    incidentTypeQuestion: string;
    incidentTypeMissingChild: string;
    incidentTypeOther: string;
    missingChildHelper: string;
    missingChildPlaceholder: string;
    otherHelper: string;
    otherPlaceholder: string;
    whatIsHappening: string;
    validationEmpty: string;
    validationTooLong: string;
    photoLabel: string;
    photoHelper: string;
    removePhoto: string;
    addPhoto: string;
    photoInvalidType: string;
    sending: string;
    sendReport: string;
    anonymousNote: string;
    charCount: (count: number, max: number) => string;
  };
  statusBanner: {
    offline: string;
    sending: (count: number) => string;
  };
  confirmation: {
    reportReceivedTitle: string;
    reportReceivedBody: string;
    reportAnother: string;
    savedOnDeviceTitle: string;
    savedOnDeviceBody: string;
  };
  receipts: {
    show: (count: number) => string;
    hide: (count: number) => string;
    timeJustNow: string;
    timeMinutesAgo: (n: number) => string;
    timeHoursAgo: (n: number) => string;
    timeDaysAgo: (n: number) => string;
  };
  footer: {
    tagline: string;
    copyright: (year: number) => string;
    consoleLink: string;
    switchToEnglish: string;
    switchToFrench: string;
  };
  alerts: {
    checking: string;
    turnedOffTitle: string;
    turnedOffBody: string;
    turnBackOn: string;
    manageHeading: string;
    onForDevice: string;
    turnOff: string;
    signupHeading: string;
    signupSubtitle: string;
    saveChanges: string;
    enableAlerts: string;
    saving: string;
    permissionNote: string;
    errorPermissionDenied: string;
    errorUnsupported: string;
    errorUnexpected: string;
  };
  alertRules: {
    alertType: string;
    incidentTypeMissingChild: string;
    incidentTypeOtherProtection: string;
    chooseAtLeastOne: string;
    minimumSeverity: string;
    severityLow: string;
    severityMedium: string;
    severityHigh: string;
    severityCritical: string;
    area: string;
    areaHelper: string;
    areaPlaceholder: string;
    areaRequired: string;
    alertTypePlaceholder: string;
    removeIncidentType: (label: string) => string;
    removeArea: (area: string) => string;
  };
}

export const translations: Record<Locale, Translations> = {
  en: {
    brand: {
      citizenBadge: "Citizen",
    },
    tabs: {
      report: "Report",
      alerts: "Get alerts",
    },
    reportForm: {
      heading: "Report an incident",
      subtitle: "No account needed. Sentinel does not collect identity fields from reporters.",
      incidentTypeQuestion: "What kind of report is this?",
      incidentTypeMissingChild: "Missing child",
      incidentTypeOther: "Other incident",
      missingChildHelper:
        "Describe the child and the situation: name, age, what they look like, where and when they were last seen. Every detail helps.",
      missingChildPlaceholder:
        "Example: My 8-year-old daughter Amina has not returned from school. She was last seen near Carrefour Bonamoussadi around 3pm today, wearing a blue school uniform...",
      otherHelper:
        "Describe what happened: who is involved, what you saw, and where and when it happened. Every detail helps.",
      otherPlaceholder:
        "Example: I saw a child being physically abused near the Bonamoussadi market around 5pm today...",
      whatIsHappening: "What is happening?",
      validationEmpty: "Please describe the situation.",
      validationTooLong: "Please shorten this a little.",
      photoLabel: "Photo (optional)",
      photoHelper:
        "A recent photo helps a reviewer confirm the report. You can skip this if you don't have one.",
      removePhoto: "Remove photo",
      addPhoto: "Add a photo",
      photoInvalidType: "Please choose a JPEG, PNG, or WebP photo.",
      sending: "Sending...",
      sendReport: "Send report",
      anonymousNote: "This report is anonymous. No account or personal information is required.",
      charCount: (count, max) => `${count.toLocaleString()} / ${max.toLocaleString()}`,
    },
    statusBanner: {
      offline:
        "You're offline. You can still fill out a report -- it will send automatically once you're connected.",
      sending: (count) => `Sending ${count} saved ${count === 1 ? "report" : "reports"}...`,
    },
    confirmation: {
      reportReceivedTitle: "Report received",
      reportReceivedBody: "Save this reference code. You can use it to follow up.",
      reportAnother: "Report another",
      savedOnDeviceTitle: "Saved on this device",
      savedOnDeviceBody:
        "You're offline right now, so this report is saved and will send automatically as soon as you're connected. You don't need to do anything else -- keep this app open or come back to it later.",
    },
    receipts: {
      show: (count) => `Show reports sent from this device (${count})`,
      hide: (count) => `Hide reports sent from this device (${count})`,
      timeJustNow: "just now",
      timeMinutesAgo: (n) => `${n}m ago`,
      timeHoursAgo: (n) => `${n}h ago`,
      timeDaysAgo: (n) => `${n}d ago`,
    },
    footer: {
      tagline: "Privacy-first civic-protection platform",
      copyright: (year) => `© ${year} ${BRAND_NAME}`,
      consoleLink: "Organization or reviewer? Sign in to the portal",
      switchToEnglish: "Switch to English",
      switchToFrench: "Switch to French",
    },
    alerts: {
      checking: "Checking your alert settings...",
      turnedOffTitle: "Alerts turned off",
      turnedOffBody: "You will no longer receive alerts on this device.",
      turnBackOn: "Turn alerts back on",
      manageHeading: "Manage your alerts",
      onForDevice: "Alerts are on for this device.",
      turnOff: "Turn off alerts",
      signupHeading: "Get missing-child alerts",
      signupSubtitle: "Choose what you want to hear about. You can change this anytime.",
      saveChanges: "Save changes",
      enableAlerts: "Enable alerts",
      saving: "Saving...",
      permissionNote:
        "Your browser will ask permission to send notifications. No account or personal information is required.",
      errorPermissionDenied:
        "Notifications were not allowed. You can enable them in your browser's site settings and try again.",
      errorUnsupported: "This browser does not support push notifications.",
      errorUnexpected: "An unexpected error occurred.",
    },
    alertRules: {
      alertType: "Alert type",
      incidentTypeMissingChild: "Missing child",
      incidentTypeOtherProtection: "Other protection incident",
      chooseAtLeastOne: "Choose at least one alert type.",
      minimumSeverity: "Minimum severity",
      severityLow: "Low and above",
      severityMedium: "Medium and above",
      severityHigh: "High and above",
      severityCritical: "Critical only",
      area: "Area",
      areaHelper: 'A city, neighborhood, or region name -- e.g. "Douala" or "Bonamoussadi".',
      areaPlaceholder: "Douala",
      areaRequired: "Please enter an area.",
      alertTypePlaceholder: "Add another alert type...",
      removeIncidentType: (label) => `Remove ${label}`,
      removeArea: (area) => `Remove ${area}`,
    },
  },
  fr: {
    brand: {
      citizenBadge: "Citoyen",
    },
    tabs: {
      report: "Signaler",
      alerts: "Recevoir des alertes",
    },
    reportForm: {
      heading: "Signaler un incident",
      subtitle: "Aucun compte requis. Sentinel ne collecte aucune donnée d'identité auprès des personnes qui signalent.",
      incidentTypeQuestion: "De quel type de signalement s'agit-il ?",
      incidentTypeMissingChild: "Enfant disparu",
      incidentTypeOther: "Autre incident",
      missingChildHelper:
        "Décrivez l'enfant et la situation : nom, âge, apparence, lieu et heure de la dernière fois qu'il a été vu. Chaque détail compte.",
      missingChildPlaceholder:
        "Exemple : Ma fille Amina, 8 ans, n'est pas rentrée de l'école. Elle a été vue pour la dernière fois près du carrefour de Bonamoussadi vers 15h aujourd'hui, portant un uniforme scolaire bleu...",
      otherHelper:
        "Décrivez ce qui s'est passé : qui est impliqué, ce que vous avez vu, ainsi que le lieu et l'heure. Chaque détail compte.",
      otherPlaceholder:
        "Exemple : J'ai vu un enfant se faire maltraiter physiquement près du marché de Bonamoussadi vers 17h aujourd'hui...",
      whatIsHappening: "Que se passe-t-il ?",
      validationEmpty: "Veuillez décrire la situation.",
      validationTooLong: "Veuillez raccourcir un peu ce texte.",
      photoLabel: "Photo (facultatif)",
      photoHelper:
        "Une photo récente aide un examinateur à confirmer le signalement. Vous pouvez l'omettre si vous n'en avez pas.",
      removePhoto: "Retirer la photo",
      addPhoto: "Ajouter une photo",
      photoInvalidType: "Veuillez choisir une photo JPEG, PNG ou WebP.",
      sending: "Envoi en cours...",
      sendReport: "Envoyer le signalement",
      anonymousNote:
        "Ce signalement est anonyme. Aucun compte ni information personnelle n'est requis.",
      charCount: (count, max) => `${count.toLocaleString("fr-FR")} / ${max.toLocaleString("fr-FR")}`,
    },
    statusBanner: {
      offline:
        "Vous êtes hors ligne. Vous pouvez tout de même remplir un signalement -- il sera envoyé automatiquement dès que vous serez reconnecté.",
      sending: (count) =>
        `Envoi de ${count} signalement${count === 1 ? "" : "s"} enregistré${count === 1 ? "" : "s"}...`,
    },
    confirmation: {
      reportReceivedTitle: "Signalement reçu",
      reportReceivedBody: "Conservez ce code de référence. Vous pourrez l'utiliser pour un suivi.",
      reportAnother: "Faire un autre signalement",
      savedOnDeviceTitle: "Enregistré sur cet appareil",
      savedOnDeviceBody:
        "Vous êtes actuellement hors ligne, donc ce signalement est enregistré et sera envoyé automatiquement dès que vous serez reconnecté. Vous n'avez rien d'autre à faire -- gardez cette application ouverte ou revenez-y plus tard.",
    },
    receipts: {
      show: (count) => `Afficher les signalements envoyés depuis cet appareil (${count})`,
      hide: (count) => `Masquer les signalements envoyés depuis cet appareil (${count})`,
      timeJustNow: "à l'instant",
      timeMinutesAgo: (n) => `il y a ${n} min`,
      timeHoursAgo: (n) => `il y a ${n} h`,
      timeDaysAgo: (n) => `il y a ${n} j`,
    },
    footer: {
      tagline: "Plateforme de protection civile axée sur la confidentialité",
      copyright: (year) => `© ${year} ${BRAND_NAME}`,
      consoleLink: "Organisation ou examinateur ? Connectez-vous au portail",
      switchToEnglish: "Passer à l'anglais",
      switchToFrench: "Passer au français",
    },
    alerts: {
      checking: "Vérification de vos paramètres d'alerte...",
      turnedOffTitle: "Alertes désactivées",
      turnedOffBody: "Vous ne recevrez plus d'alertes sur cet appareil.",
      turnBackOn: "Réactiver les alertes",
      manageHeading: "Gérer vos alertes",
      onForDevice: "Les alertes sont activées pour cet appareil.",
      turnOff: "Désactiver les alertes",
      signupHeading: "Recevoir les alertes enfant disparu",
      signupSubtitle:
        "Choisissez ce que vous souhaitez suivre. Vous pouvez modifier ce choix à tout moment.",
      saveChanges: "Enregistrer les modifications",
      enableAlerts: "Activer les alertes",
      saving: "Enregistrement...",
      permissionNote:
        "Votre navigateur vous demandera l'autorisation d'envoyer des notifications. Aucun compte ni information personnelle n'est requis.",
      errorPermissionDenied:
        "Les notifications n'ont pas été autorisées. Vous pouvez les activer dans les paramètres du site de votre navigateur et réessayer.",
      errorUnsupported: "Ce navigateur ne prend pas en charge les notifications push.",
      errorUnexpected: "Une erreur inattendue s'est produite.",
    },
    alertRules: {
      alertType: "Type d'alerte",
      incidentTypeMissingChild: "Enfant disparu",
      incidentTypeOtherProtection: "Autre incident de protection",
      chooseAtLeastOne: "Choisissez au moins un type d'alerte.",
      minimumSeverity: "Gravité minimale",
      severityLow: "Faible et plus",
      severityMedium: "Moyenne et plus",
      severityHigh: "Élevée et plus",
      severityCritical: "Critique uniquement",
      area: "Zone",
      areaHelper:
        'Un nom de ville, de quartier ou de région -- ex. "Douala" ou "Bonamoussadi".',
      areaPlaceholder: "Douala",
      areaRequired: "Veuillez saisir une zone.",
      alertTypePlaceholder: "Ajouter un autre type d'alerte...",
      removeIncidentType: (label) => `Retirer ${label}`,
      removeArea: (area) => `Retirer ${area}`,
    },
  },
};

public class Personne {
    private String nom;
    private int age;

    public Personne(String nom, int age) {
        this.nom = nom;
        this.age = age;
    }

    public String getNom() {
        return nom;
    }

    public int getAge() {
        return age;
    }

    public void setNom(String nom) {
        this.nom = nom;
    }

    public void setAge(int age) {
        this.age = age;
    }

    public void feterAnniversaire() {
        this.age++;
        System.out.println(this.nom + " a maintenant " + this.age + " ans. Joyeux anniversaire !");
    }

    public void saluer() {
        System.out.println("Bonjour, je m'appelle " + this.nom + " et j'ai " + this.age + " ans.");
    }

    @Override
    public String toString() {
        return nom + " (Age: " + age + ")";
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        Personne personne = (Personne) o;
        return age == personne.age && Objects.equals(nom, personne.nom);
    }

    @Override
    public int hashCode() {
        return Objects.hash(nom, age);
    }
}

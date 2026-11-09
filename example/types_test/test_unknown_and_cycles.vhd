package p is
    type t_a is access t_b;
    type t_b is access t_a;
    type t_rec is record
        f : t_missing;
    end record;
end package p;

use work.p.all;

entity top is
    port (
        x : out t_a;
        y : out t_rec
    );
end entity top;
